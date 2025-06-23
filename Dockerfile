FROM rust:1.87-slim-bookworm as build
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config curl libssl-dev

WORKDIR /app
COPY . /app
RUN cargo build --release

# -----

FROM debian:bookworm-slim as production

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl3

RUN groupadd --gid 10001 app && \
    useradd -g app --uid 10001 --shell /usr/sbin/nologin --no-create-home --home-dir /app app

WORKDIR /app

COPY --from=build /app/target/release/classify-client .
COPY --from=build /app/version.json /app/GeoLite2-Country.mmdb* ./

USER app
ENV PORT=8000
ENV HOST=0.0.0.0
EXPOSE $PORT

CMD ["/app/classify-client"]
