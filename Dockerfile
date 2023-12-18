FROM messense/rust-musl-cross:x86_64-musl AS builder

# use nightly to build
RUN rustup update nightly && \
    rustup override set nightly && \
    rustup target add --toolchain nightly x86_64-unknown-linux-musl

# install sqlx
ENV SQLX_OFFLINE true
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install sqlx-cli --no-default-features --features rustls,postgres

WORKDIR /home/rust/src
COPY . .

# generate binaries and drop them in build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install --locked --path . --root /build/

# build the final image
FROM debian:buster-slim
RUN apt-get update & apt-get install -y extra-runtime-dependencies & rm -rf /var/lib/apt/lists/*


COPY --from=builder /build/bin/* /usr/local/bin/
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx

# we still need all the templates, migrations, etc.
WORKDIR /home/rust/src
COPY . .

CMD ["server"]

