# Security policy

Harness Manager handles other people's logins and API keys, so security reports get priority.

## Reporting a vulnerability

Report it privately through GitHub: **Security → Report a vulnerability** on this repository
([direct link](https://github.com/abushanab-net/HarnessManager/security/advisories/new)).
Please do not open a public issue for a security problem.

Include what you found, how to reproduce it, and which version you used (Settings → Updates shows it).
You will get an answer within a week. A fix ships in a new release, and the advisory is published once
installed copies have had the chance to update.

## Supported versions

Only the newest release gets security fixes. Installed copies update themselves (Settings → Updates).

## What the app promises

These are the rules a report would show broken:

- Subscription logins are never refreshed, copied or stored. The app only reads what each tool saved.
- API keys live only in the operating system's credential store (Windows Credential Manager). They never go
  into files, command-line arguments, logs or backups.
- The local router listens on 127.0.0.1 only and refuses any request without its token.
- Updates install only when signed by the release key.
- Harness Manager only removes MCP servers, skills and settings that its own ledger says it wrote.
