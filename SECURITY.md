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

- Subscription logins are never refreshed or sent anywhere. The app reads what each tool saved. One exception:
  Antigravity has a single login for the whole computer, so Harness Manager keeps a copy of each Antigravity
  account's login in the credential store and puts it in place when that account starts. That swap exists on
  Windows only: on macOS and Linux Antigravity runs as its one account, and Harness Manager keeps no copy of its
  login.
- API keys, and the env and header values of MCP servers, live only in the operating system's credential store:
  Windows Credential Manager, the Keychain on macOS, the Secret Service on Linux (GNOME Keyring, KWallet). They
  never go into command-line arguments, logs or backups. The account list holds an empty marker in their place,
  with one exception: an MCP value the store refuses (too large for one credential, for example) stays in the
  account list, and the log says so. This computer's device key, ring key and hub token are kept in the same
  store.
- On macOS and Linux, a computer where that store cannot be reached (a Linux server with no desktop session, a
  Mac whose keychain is locked) keeps the same secrets in one file, `secrets.json` in the data folder, which only its owner can read
  (mode 0600). The values in it are encoded, not encrypted: the file's permissions are its whole protection. It
  is never in a backup, and `hrm secrets check` says which of the two a computer uses.
- What another computer, the shared folder or the hub sends is checked before it is used: names must be plain
  names, sizes have a limit, and another computer's journal can change only its own rows.
- The environment switches the test suite uses are ignored by an installed copy working on its own data folder.
- The local router listens on 127.0.0.1 only and refuses any request without its token.
- Updates install only when signed by the release key.
- Harness Manager only removes MCP servers, skills and settings that its own ledger says it wrote.
