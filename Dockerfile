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
    cargo build -r

COPY Rocket.toml Rocket.toml
COPY .sqlx .sqlx
COPY src src/
COPY migrations migrations/
COPY assets assets/
COPY fixtures fixtures/
COPY templates templates/

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build -r

CMD ["target/release/server"]
