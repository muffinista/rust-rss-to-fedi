# syntax=docker/dockerfile:experimental

FROM rustlang/rust:nightly

WORKDIR /app

ENV SQLX_OFFLINE true

# COPY Cargo.toml Cargo.lock .
COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo install sqlx-cli --no-default-features --features rustls,postgres && \
    cargo fetch

RUN echo "==============================="
RUN cargo build

# CMD ["cargo", "run", "--bin", "server"]
RUN find /app

CMD ["target/debug/server"]
