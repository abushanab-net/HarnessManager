# Phone push relay

Run one relay beside the hub on the owner's Docker server. The hub's Caddy handles HTTPS. The relay has no published host port and writes no request or provider access logs.

## Start it

From the repository root on the server, with the hub already running and DNS for `push.example.com` pointing to it:

```sh
sudo install -d -m 0750 -o root -g 65532 /etc/hrm-push
sudo install -m 0640 -o root -g 65532 deploy/push-relay/config.example.json /etc/hrm-push/config.json
# Create the signing key once. Preserve it across restarts and restores.
sudo sh -c 'set -C; umask 077; openssl rand -hex 32 > /etc/hrm-push/relay.key'
sudo chgrp 65532 /etc/hrm-push/relay.key
sudo chmod 0640 /etc/hrm-push/relay.key
docker compose -f deploy/push-relay/docker-compose.yml up -d --build
```

The compose file joins the existing `hub_default` network. If the hub uses a custom project name, set `HUB_NETWORK` to the name shown by `docker network ls`. Set `PUSH_CONFIG_DIR` to override the host configuration directory. Inside the image, `PUSH_CONFIG` defaults to `/etc/hrm-push/config.json`.

Append a site to the hub's mounted Caddyfile, substituting the real hostname. Keep request and debug logging disabled here and at upstream proxies. The error handler returns a generic response if Caddy cannot reach the relay, so a failed deletion's URL stays out of Caddy's normal error logs.

```caddyfile
push.example.com {
    reverse_proxy hrm-push-relay:8788
    handle_errors {
        respond "Push relay unavailable" 503
    }
}
```

```sh
docker compose -f deploy/hub/docker-compose.yml exec caddy caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile
curl --fail https://push.example.com/health
```

Set **Settings → Connections → Phone push relay** on each desktop to the HTTPS origin. Clear the field to disable wakes. A signed phone enrollment takes effect on the next request. A provider outage or a slow relay leaves the hook free to take a local answer.

## Owner credentials

The example starts without provider accounts. Health, registration and removal work in this configuration. A wake for an unconfigured provider returns HTTP 503 and keeps the registration.

- **Firebase**. Enable Firebase Cloud Messaging HTTP v1 for the app's Google Cloud project. Supply a service-account JSON with permission `cloudmessaging.messages.create`. Set `fcm_service_account_file` to its mounted path, e.g. `"/etc/hrm-push/firebase.json"`. [Firebase authorization](https://firebase.google.com/docs/cloud-messaging/send/v1-api)
- **Apple**. Supply an APNs token-signing .p8 key, its key ID and team ID from an Apple Developer membership with push enabled for `net.abushanab.harnessmanager`. Set `apns` to `{"key_file":"/etc/hrm-push/AuthKey.p8","key_id":"YOUR_KEY_ID","team_id":"YOUR_TEAM_ID","sandbox":true}`. Choose the environment matching the device's entitlement. Alerts use the bundle ID as the topic. Live updates use the activity's push token, the `.push-type.liveactivity` topic suffix and an empty ActivityKit content state. Register the activity token separately and send only live wakes to its handle. [APNs token authentication](https://developer.apple.com/documentation/usernotifications/establishing-a-token-based-connection-to-apns) · [ActivityKit push updates](https://developer.apple.com/videos/play/wwdc2023/10185/)

Keep keys outside the repository and readable by the container's UID/GID 65532. Restart the relay after changing configuration. Back up the signing key with the SQLite volume. Replacing the key requires phones to register again.

## Wire contract

1. The phone posts `{"platform":"android","token":"..."}` or `{"platform":"ios","token":"..."}` to `/v1/register`. HTTP 201 returns a random `relay_handle` and a 32-byte hex `secret`. Transfer both fields to the enrolled computer over mutual TLS using `PUT /c1/push` (`Client::push_register_signed`). A legacy handle alone cannot sign a wake.
2. Post `{"relay_handle":"...","kind":"needs","collapse_id":"..."}` to `/v1/wake`. Use an opaque collapse ID of 1–64 ASCII letters, digits, underscores or hyphens. Use `live` for a data or ActivityKit update. An alert contains only “A session needs you”.
3. Sign wakes and `DELETE /v1/register/{handle}` using `X-HRM-Timestamp` (Unix milliseconds) and `X-HRM-Signature` (hex HMAC-SHA256). Decode the secret from hex. The exact signed bytes are the uppercase HTTP method, a newline, the absolute request path, a newline, the decimal timestamp, a newline and the raw UTF-8 body (empty on DELETE). Send the same serialized bytes that were signed.
4. Synchronize sender and relay clocks. Timestamps must be less than 60 seconds old and at most five seconds ahead. After a restart with stored handles, sign after the relay's startup time plus five seconds. This excludes every signature the previous instance could have accepted. The relay remembers accepted signatures in memory, including failed delivery attempts. A repeated wake gets 409. Each handle has ten accepted attempts per rolling minute. Excess wakes get 429. A fresh signed deletion still works.
5. HTTP 204 means the provider accepted the wake or the registration was removed. A permanent token rejection removes that handle and returns 410. Unavailable providers return 503. The desktop sends best effort with a two-second timeout and no application retries. To turn off an enrollment, remove it from the computer with `DELETE /c1/push`. To retire a relay handle entirely, send the signed relay DELETE.

SQLite persists only the random handle, platform and device token. The relay derives each handle's secret with HMAC-SHA256 under the signing key. Run one instance per database so one replay and rate-limit window covers every wake.
