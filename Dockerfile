FROM rust:1.45-slim-buster AS builder
RUN apt-get update \
	&& apt-get install -y libssl-dev pkg-config \
	&& rm -rf /var/lib/apt/lists/*
WORKDIR /usr/src/hitide
COPY Cargo.* ./
COPY src ./src
COPY res ./res
RUN cargo build --release

FROM debian:buster-slim
RUN apt-get update \
	&& apt-get install -y openssl \
	&& rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/hitide/target/release/hitide /usr/bin/
CMD ["hitide"]
