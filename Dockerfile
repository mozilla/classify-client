FROM rust:1.31-slim-stretch as build
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config curl libssl-dev

WORKDIR /app
COPY . /app
RUN cargo build --release

# -----

FROM debian:stretch-slim as production

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl1.1

WORKDIR /app

COPY --from=build /app/target/release/classify-client .
COPY --from=build /app/GeoLite2-Country.mmdb ./
COPY --from=build /app/version.json ./

ENV PORT=8080
ENV HOST=0.0.0.0
EXPOSE $PORT

CMD ["/app/classify-client"]
