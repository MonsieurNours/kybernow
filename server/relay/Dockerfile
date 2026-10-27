# syntax=docker/dockerfile:1
# Relais de KyberNow (README.md) : compilé dans l'image Rust, puis le
# binaire seul sur distroless, sans root. Contexte : ce dossier.
#   docker build -t kybernow-relay .

FROM rust:1.89-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked --bin kybernow-relay

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/kybernow-relay /usr/local/bin/kybernow-relay
USER nonroot:nonroot
# Un seul port, en TCP et en UDP.
EXPOSE 8443/tcp 8443/udp
# /healthz sur l'adresse des métriques (127.0.0.1:9464 par défaut).
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD ["/usr/local/bin/kybernow-relay", "--healthcheck"]
ENTRYPOINT ["/usr/local/bin/kybernow-relay"]
