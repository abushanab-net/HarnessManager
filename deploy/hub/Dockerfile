# The hub in one small image. Build from the repository root:
#   docker build -f deploy/hub/Dockerfile -t ghcr.io/abushanab-net/hrm-hub:dev .
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p hrm-hub && mkdir -p /data

# distroless: libc and libgcc only, no shell; runs as the unprivileged `nonroot` user, which owns /data
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/hrm-hub /hrm-hub
COPY --from=build --chown=nonroot:nonroot /data /data
ENV HUB_DATA=/data HUB_BIND=0.0.0.0:8787
VOLUME /data
EXPOSE 8787
ENTRYPOINT ["/hrm-hub"]
