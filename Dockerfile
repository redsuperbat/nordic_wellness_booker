FROM rust AS builder

WORKDIR /app

COPY . .

RUN cargo build --release

# and then copy it to an empty docker image
FROM debian

WORKDIR /app

COPY --from=builder /app/target/release/nordic_wellness_booker .

CMD ["./nordic_wellness_booker"]