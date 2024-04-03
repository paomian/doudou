FROM fedora-minimal
COPY target/release/doudou /doudou
COPY config.json /config.json
ENTRYPOINT ["/doudou", "-f", "/config.json"]
