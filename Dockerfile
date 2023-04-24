FROM rust AS builder

WORKDIR /app

COPY . .

RUN cargo build --release

# and then copy it to an empty docker image
FROM debian

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates

RUN update-ca-certificates

WORKDIR /app

COPY --from=builder /app/target/release/nordic_wellness_booker .

ENV TZ=Europe/Stockholm
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

CMD ["./nordic_wellness_booker"]