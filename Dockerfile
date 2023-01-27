# syntax=docker/dockerfile:experimental

FROM rustlang/rust:nightly

WORKDIR /app
# COPY . .

# ENV DATABASE_URL sqlite:build.sqlite
# need DATABASE_URL and DOMAIN_NAME to actually run things

COPY Cargo.toml Cargo.lock .

# RUN cargo install sqlx-cli --no-default-features --features native-tls,sqlite
# COPY migrations migrations
# RUN rm -f build.sqlite && sqlx database setup 

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    # cargo install --path . && \
    cargo install sqlx-cli --no-default-features --features rustls,postgres && \
    cargo build

CMD ["cargo", "run", "--bin", "server"]
