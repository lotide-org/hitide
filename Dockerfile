FROM rust:1.45-alpine AS builder
RUN apk add --no-cache cargo openssl-dev
WORKDIR /usr/src/hitide
COPY Cargo.* ./
COPY src ./src
COPY res ./res
RUN cargo build --release

FROM alpine:3.12
RUN apk add --no-cache libgcc openssl
COPY --from=builder /usr/src/hitide/target/release/hitide /usr/bin/
CMD ["hitide"]
