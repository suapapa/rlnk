FROM rust:1.90-alpine AS builder
WORKDIR /app

RUN apk add --no-cache build-base ca-certificates

COPY Cargo.toml Cargo.lock ./
COPY src ./src

ENV RUSTFLAGS="-C strip=symbols"

RUN cargo build --release --locked

FROM scratch AS runtime

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/target/release/rlnk /usr/local/bin/rlnk

USER 65532:65532
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/rlnk"]
