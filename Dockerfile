FROM rust:1 AS builder
COPY . .
RUN cargo build --release --bin nordic_wellness_booker

FROM ubuntu:latest
WORKDIR /app
COPY --from=builder /app/target/release/nordic_wellness_booker .
CMD ["./nordic_wellness_booker"]