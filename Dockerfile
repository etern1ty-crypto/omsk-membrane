FROM rust:1.85.1-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY gulag ./gulag
COPY synapse ./synapse
COPY reactor ./reactor
COPY .env.example ./
RUN cargo build --offline --locked --release -p reactor --bin omsk

FROM debian:bookworm-slim AS runtime
RUN groupadd --gid 10001 omsk && useradd --uid 10001 --gid 10001 --no-create-home omsk
COPY --from=build /src/target/release/omsk /usr/local/bin/omsk
USER 10001:10001
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/omsk"]
CMD ["--help"]
