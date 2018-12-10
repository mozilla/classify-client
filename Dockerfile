FROM rust:1.31-slim-stretch as build

WORKDIR /app
COPY . /app
RUN cargo build --release

# -----

FROM debian:stretch-slim as production
WORKDIR /app

COPY --from=build /app/target/release/classify-client .
COPY ./GeoLite2-Country.mmdb ./
ENV PORT=8080
ENV HOST=0.0.0.0
EXPOSE $PORT

CMD ["/app/classify-client"]