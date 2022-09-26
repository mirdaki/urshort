FROM rust:1.63.0 AS chef
RUN cargo install cargo-chef
WORKDIR app

FROM chef AS prepper
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder
COPY --from=prepper /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin urshort

FROM debian:bullseye-slim AS runtime
WORKDIR /usr/local/bin
COPY --from=builder /app/target/release/urshort /usr/local/bin
ccccckl

EXPOSE 54027
ENTRYPOINT ["/usr/local/bin/urshort"]
