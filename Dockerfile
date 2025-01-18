FROM rust AS builder
WORKDIR /app
COPY *.toml .
COPY Cargo.lock .
COPY ./src ./src
COPY ./templates ./templates
COPY ./migrations ./migrations
RUN cargo build --release

FROM debian:stable-slim AS runner
RUN mkdir -p /app/db
WORKDIR /app
COPY --from=builder /app/target/release/wol-gui /app/wol-gui
COPY ./templates /app/templates
COPY ./Rocket.toml /app
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8000
EXPOSE 8000
VOLUME /app/db
CMD ["/app/wol-gui"]