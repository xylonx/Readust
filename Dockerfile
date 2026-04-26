FROM clux/muslrust:stable AS builder

WORKDIR /app

COPY . .

RUN set -x && cargo build --target x86_64-unknown-linux-musl --release --locked

FROM scratch

WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/readust /app/readust

CMD ["/app/readust"]
