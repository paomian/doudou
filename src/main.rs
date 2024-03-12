use clap::Parser;
use libc::{
    fcntl, ioctl, isatty, read, tcflush, tcgetattr, tcsetattr, termios, termios2, BOTHER, CBAUD,
    CLOCAL, CREAD, CRTSCTS, CS8, CSIZE, CSTOPB, F_GETFL, F_SETFL, O_NDELAY, O_NOCTTY, O_NONBLOCK,
    O_RDWR, TCGETS2, TCIOFLUSH, TCSANOW, TCSETS2, VMIN, VTIME,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::str::FromStr;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::{Builder, Runtime};

// TODO add error handling

fn set_opt(fd: i32, baudrate: u32) {
    let mut new_tio = termios {
        c_iflag: 0,
        c_oflag: 0,
        c_cflag: 0,
        c_lflag: 0,
        c_line: 0,
        c_cc: [0; 32],
        c_ispeed: 0,
        c_ospeed: 0,
    };
    let tg_r = unsafe { tcgetattr(fd, &mut new_tio) };
    if tg_r < 0 {
        println!("error tcgetattr.");
    }
    new_tio.c_cflag = new_tio.c_cflag | CLOCAL | CREAD;
    new_tio.c_cflag &= !CSIZE;
    new_tio.c_cflag |= CS8;
    new_tio.c_cflag &= !CSTOPB;
    new_tio.c_cflag |= CRTSCTS;
    new_tio.c_cc[VMIN] = 0;
    new_tio.c_cc[VTIME] = 10;
    unsafe { tcflush(fd, TCIOFLUSH) };

    let tc_r = unsafe { tcsetattr(fd, TCSANOW, &new_tio) };
    if tc_r < 0 {
        println!("error tcsetattr.");
    }
    set_custom_baudrate(fd, baudrate);
}

fn set_custom_baudrate(fd: i32, baudrate: u32) {
    let mut tio = termios2 {
        c_iflag: 0,
        c_oflag: 0,
        c_cflag: 0,
        c_lflag: 0,
        c_line: 0,
        c_cc: [0; 19],
        c_ispeed: 0,
        c_ospeed: 0,
    };
    let tcg_r = unsafe { ioctl(fd, TCGETS2, &tio) };
    if tcg_r < 0 {
        println!("TCGETS2");
    }
    tio.c_cflag &= !CBAUD;
    tio.c_cflag |= BOTHER;
    tio.c_ispeed = baudrate;
    tio.c_ospeed = baudrate;

    let tcs_r = unsafe { ioctl(fd, TCSETS2, &tio) };
    if tcs_r < 0 {
        println!("TCSETS2");
    }
    let tcg_r = unsafe { ioctl(fd, TCGETS2, &tio) };
    if tcg_r < 0 {
        println!("TCGETS2");
    }
}

fn calc_tvoc(h: u8, l: u8) -> Decimal {
    (Decimal::new(h.into(), 0) * dec!(256) + Decimal::new(l.into(), 0)) * dec!(0.001)
}

fn calc_co2(h: u8, l: u8) -> Decimal {
    Decimal::new(h.into(), 0) * dec!(256) + Decimal::new(l.into(), 0)
}

struct CalcResult {
    tvoc: Decimal,
    ch2o: Decimal,
    co2: Decimal,
}

fn calc_value(data: &[u8]) -> CalcResult {
    let tvoc_h = data[2];
    let tvoc_l = data[3];
    let ch2o_h = data[4];
    let ch2o_l = data[5];
    let co2_h = data[6];
    let co2_l = data[7];
    let tvoc = calc_tvoc(tvoc_h, tvoc_l);
    let ch2o = calc_tvoc(ch2o_h, ch2o_l);
    let co2 = calc_co2(co2_h, co2_l);
    CalcResult { tvoc, ch2o, co2 }
}

fn timestamp_m() -> u128 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_millis()
}

fn read_data(fd: i32, rt: &Runtime, tx: tokio::sync::mpsc::Sender<AirInfo>) {
    let mut buffer = [0u8; 1024];
    let n_read = unsafe { read(fd, buffer.as_mut_ptr() as *mut libc::c_void, 1024) };
    if n_read < 0 {
        println!("error read.");
    } else {
        let result = calc_value(&buffer);
        let now = timestamp_m();
        rt.spawn(async move {
            tx.send(AirInfo {
                info: result,
                timestamp: now,
            })
            .await
            .unwrap();
        });
    }
}

struct AirInfo {
    info: CalcResult,
    timestamp: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ConnectionInfo {
    host: String,
    dbname: String,
    username: String,
    password: String,
}

impl FromStr for ConnectionInfo {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    device: String,
    conn: ConnectionInfo,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let device = args.device;
    let conn = args.conn;
    let flags = O_RDWR | O_NOCTTY | O_NDELAY;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(&device)?;
    let fd = file.as_raw_fd();
    let file_flag = unsafe { fcntl(fd, F_GETFL) };
    let file_flag = file_flag & !O_NONBLOCK;
    let set_result = unsafe { fcntl(fd, F_SETFL, file_flag) };
    if set_result < 0 {
        println!("error set.");
    }
    let is_tty = unsafe { isatty(fd) };
    if is_tty == 0 {
        println!("no tty.");
    }
    set_opt(fd, 9600);

    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<AirInfo>(100);
    runtime.spawn(async move {
        let client = reqwest::Client::new();
        let mut count = 0;
        while let Some(air_info) = rx.recv().await {
            let dbname = conn.dbname.clone();
            let username = conn.username.clone();
            let password = conn.password.clone();
            let url = format!("https://{}/v1/sql", conn.host);
            let params = [("db", dbname)];
            let url_with_params = reqwest::Url::parse_with_params(&url, &params);
            if let Ok(final_url) = url_with_params {
                let sql = format!(
                    "insert into air (ts,tvoc,cho2,co2) values ({},{},{},{})",
                    air_info.timestamp, air_info.info.tvoc, air_info.info.ch2o, air_info.info.co2
                );
                let form_params = [("sql", &sql)];
                let res = client
                    .post(final_url)
                    .basic_auth(username, Some(password))
                    .form(&form_params)
                    .send()
                    .await;
                match res {
                    Ok(_) => {
                        count += 1;
                        if count % 10 == 0 {
                            println!("send data to server: {}", count);
                        }
                    }
                    Err(e) => println!("error: {}", e),
                }
            } else {
                println!("error parse url {}, params: {:?}", url, params);
            }
        }
    });
    loop {
        let tx_c = tx.clone();
        sleep(Duration::from_secs(3));
        read_data(fd, &runtime, tx_c);
    }
    // Ok(())
}
