FROM rust:1 as builder
WORKDIR /app
COPY . .
# build the binary and install it at /usr/local/cargo/bin/
RUN cargo install --path .

FROM debian:buster-slim as runner
COPY --from=builder /usr/local/cargo/bin/rusty-dusty /usr/local/bin/rusty-dusty
ENV ROCKET_ADDRESS=0.0.0.0
EXPOSE 8000
CMD ["rusty-dusty"]