# Multi-stage runtime image. Framework auth and job migrations are inside the binary.
# The runtime stage does not contain the Rust source tree or the migration JSON files.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p fexu-cli --bin embedded-migrate

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/embedded-migrate /usr/local/bin/embedded-migrate
USER nobody
ENV BIND_ADDRESS=0.0.0.0:3000
ENV FEXU_ALLOW_WILDCARD_BIND=true
ENTRYPOINT ["embedded-migrate"]
