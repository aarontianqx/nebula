FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN groupadd --system transit && useradd --system --gid transit --home-dir /var/lib/transit transit \
    && mkdir -p /etc/transit /var/lib/transit/logs \
    && chown -R transit:transit /var/lib/transit \
    && chmod 700 /var/lib/transit/logs
COPY --from=build /build/target/release/transit /usr/local/bin/transit
ENV XDG_STATE_HOME=/var/lib
USER transit
EXPOSE 8787
STOPSIGNAL SIGTERM
ENTRYPOINT ["transit"]
CMD ["serve", "--config", "/etc/transit/transit.toml"]
