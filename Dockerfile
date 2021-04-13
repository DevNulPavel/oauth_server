# Сборка с помощью пакета Rust
# https://hub.docker.com/_/rust
FROM rust:1.51.0 as builder
ENV SQLX_OFFLINE true
WORKDIR /usr/src/oauth_server
COPY . ./
RUN cargo build --release

# Сборка рабочего пакета
FROM debian:10.9
WORKDIR /oauth_server
COPY --from=builder \
    /usr/src/oauth_server/target/release/oauth_server \
    oauth_server
COPY --from=builder \
    /usr/src/oauth_server/migrations \
    migrations
COPY --from=builder \
    /usr/src/oauth_server/static \
    static
COPY --from=builder \
    /usr/src/oauth_server/templates \
    templates
CMD ["./oauth_server"]