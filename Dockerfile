# syntax=docker/dockerfile:experimental

FROM rustlang/rust:nightly

ENV SQLX_OFFLINE true
RUN cargo install sqlx-cli --no-default-features --features rustls,postgres

WORKDIR /app

# create a minimal program so cargo can fetch/build deps
# without building the entire app
RUN mkdir -p src/bin && echo "fn main() {}" > src/bin/server.rs

COPY Cargo.toml Cargo.lock .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build

COPY Rocket.toml Rocket.toml
COPY sqlx-data.json sqlx-data.json
COPY src src/
COPY migrations migrations/
COPY assets assets/
COPY fixtures fixtures/
COPY templates templates/

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build

CMD ["target/debug/server"]
