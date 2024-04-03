# Sync ch343 uart air quality sensor to GrepTime Cloud

## Introduction

file description:

1. 99-air-detector.rules: udev rules for CH340/CH341 USB to serial adapter, It change the device permission to 666, so that the user can access the device without root permission. you can copy it to the `/etc/udev/rules.d/` directory, and re-plug the device, the device will be accessible without root permission.
2. Containerfile: Dockerfile for building the container image
3. config.json: configuration file for the sensor. It contains the sensor's serial device path, and the GrepTime Cloud's DB connection information

## Requirements

1. Rust(1.7.6) && Cargo
2. Linux 6.7.11-200.fc39.x86_64(maybe other version is ok,it's not tested)
3. podman(Optional)

## Usage

### Bare Metal

1. Build the rust executable file

```shell
cargo build --release
```

2. Change the configuration file, copy the example file to the current directory, and modify the content

```bash
cp config.json.example config.json
```

3. Run the executable file

```shell
./target/release/doudou -f config.json
```

### Containerize

1. Build the container image,you must build the rust executable file first

```shell
sudo podman build -t air-sync .
```

because we need to run it on the rootful mode by systemd, so we need to build the image with root permission to store the image in the system's image store.

Or you can build it with normal user and push it to the registry, then pull it with root permission like

```shell
podman save localhost/air-sync:latest -o air.tar
sudo podman load -i air.tar
```

if you want to run it on rootless mode, you can build the image with the normal user.

```shell
podman build -t air-sync .
```

2. add quadlet file `air.container` to the `/etc/containers/systemd/air.container`

```shell
sudo cp air.container /etc/containers/systemd/air.container
```

if you want to run it on rootless mode, you can add the quadlet file to the `~/.config/containers/containers.conf`

3. reload the systemd service

```shell
sudo systemctl daemon-reload
```

4. start the container

```shell
sudo systemctl start air.service
```
