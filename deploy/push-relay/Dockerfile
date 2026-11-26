FROM rust:1-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends cmake && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --locked --release -p hrm-push-relay && mkdir /data

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/hrm-push-relay /hrm-push-relay
COPY --from=build --chown=nonroot:nonroot /data /data
VOLUME /data
EXPOSE 8788
ENTRYPOINT ["/hrm-push-relay"]
