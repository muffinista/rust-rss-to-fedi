FROM rustlang/rust:nightly

WORKDIR /app
# COPY . .

ENV DATABASE_URL sqlite:build.sqlite

# need DATABASE_URL and DOMAIN_NAME to actually run things

COPY Cargo.toml Cargo.lock .
RUN cargo install sqlx-cli --no-default-features --features native-tls,sqlite
COPY migrations migrations
RUN rm -f build.sqlite && sqlx database setup 
# RUN rm -f build.sqlite && sqlite3 build.sqlite < migrations/*.sql

COPY . .
RUN cargo install --path .
RUN rm -f build.sqlite

CMD ["cargo", "run", "--bin", "server"]
