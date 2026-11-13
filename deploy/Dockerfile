FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p myshop

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends wget ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 ferrioxid
WORKDIR /home/ferrioxid
COPY --from=build /src/target/release/myshop /usr/local/bin/myshop
USER ferrioxid
ENV BIND_ADDRESS=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=3s --retries=5 CMD wget -qO- http://127.0.0.1:8080/readyz || exit 1
CMD ["myshop"]
