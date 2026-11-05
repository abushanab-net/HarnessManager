# The hub

A small service on a VPS that every computer pushes its journal to and pulls the others' from, with presence,
a rendezvous for direct connections across networks, and encrypted transcripts. It never sees a key: chunks,
presence and transcripts arrive encrypted with the ring key the computers share. It keeps the signed device
records (names, ids, public keys, project roots), sizes and times.

## A VPS in ten lines

1. A VPS with Docker (Compose v2) and ports 80 and 443 open.
2. DNS: point `hub.example.com` at it (A or AAAA).
3. `git clone` this repository, or copy this folder; `cd deploy/hub`.
4. `cp .env.example .env`; set `HUB_DOMAIN` and `HUB_ADMIN_TOKEN` (`openssl rand -hex 32`).
5. `docker compose up -d`. Caddy gets a Let's Encrypt certificate on first use.
6. `curl https://hub.example.com/health` answers `ok`.
7. On the first computer: Settings › Computers › A hub you host: the address and the admin token (or
   `hrm hub join https://hub.example.com --admin <token>`). Save the recovery key it shows.
8. On every other computer: Invite a computer… on a joined one gives a six-digit code good for ten minutes;
   join with it (`hrm hub join https://hub.example.com --code 123456`).
9. Updates: `docker compose pull && docker compose up -d`.
10. Backup: the volume `hub-data` (`/data`: `hub.db` and `blobs/`). Losing it loses nothing the computers do
    not have; they push again.

## What runs

- `hrm-hub`: plain HTTP on 8787 inside the network, SQLite at `/data/hub.db`, blobs under `/data/blobs/<id>/`.
  Environment: `HUB_ADMIN_TOKEN` (needed), `HUB_DATA` (`/data`), `HUB_BIND` (`0.0.0.0:8787`),
  `HUB_MAX_BLOB_MB` (64: a bigger chunk or transcript is refused), `HUB_RETAIN_MB` (2048 per computer:
  the oldest chunks beyond it are dropped, never the newest).
- `caddy`: TLS on 443 for `HUB_DOMAIN`, proxied to `hrm-hub:8787`.

Tokens are per computer, minted at join and stored hashed; the admin token only joins and forgets
computers. `DELETE /v1/devices/<id>` with the admin token forgets a computer that was lost.

## Build the image yourself

From the repository root: `docker build -f deploy/hub/Dockerfile -t ghcr.io/abushanab-net/hrm-hub:dev .`,
then `HUB_TAG=dev docker compose up -d`. A release tag `vX.Y.Z` publishes `ghcr.io/abushanab-net/hrm-hub:X.Y.Z`
and `latest` from the release workflow.
