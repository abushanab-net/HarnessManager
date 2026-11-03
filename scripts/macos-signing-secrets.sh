#!/usr/bin/env bash
# Sets the six GitHub secrets the release uses to sign and notarize the macOS app with a Developer ID.
# Run it on a Mac, with the certificate exported from Keychain Access as a .p12 file:
#   bash scripts/macos-signing-secrets.sh ~/Desktop/DeveloperID.p12 [owner/repo]
# Passwords are read without echo and passed to gh on stdin, so they never show on screen, in the shell
# history or in the process list. The .p12 file is deleted afterwards.
set -euo pipefail
p12="${1:?usage: $0 <DeveloperID.p12> [owner/repo]}"
repo="${2:-abushanab-net/HarnessManager}"
test -f "$p12" || { echo "no such file: $p12"; exit 1; }
command -v gh >/dev/null || { echo "Install the GitHub CLI first: brew install gh"; exit 1; }
gh auth status >/dev/null 2>&1 || gh auth login

read -rsp "Password of the .p12 file: " p12pw; echo
# the identity and team come from the certificate itself, so nothing is typed by hand
export P12PW="$p12pw"
# -legacy: OpenSSL 3 (Homebrew) needs it for the older ciphers some Keychain exports use
subject=$({ openssl pkcs12 -in "$p12" -nokeys -passin env:P12PW 2>/dev/null || openssl pkcs12 -legacy -in "$p12" -nokeys -passin env:P12PW 2>/dev/null; } \
  | openssl x509 -noout -subject -nameopt multiline 2>/dev/null) \
  || { echo "Could not open the .p12: wrong password?"; exit 1; }
identity=$(sed -n 's/^ *commonName *= *//p' <<<"$subject")
team=$(sed -n 's/^ *organizationalUnitName *= *//p' <<<"$subject")
[[ "$identity" == "Developer ID Application:"* ]] || { echo "This is \"$identity\", not a Developer ID Application certificate."; exit 1; }
[[ "$team" =~ ^[A-Z0-9]{10}$ ]] || { echo "No team ID in the certificate."; exit 1; }
echo "Certificate: $identity"

read -rp "Apple account email: " appleid
echo "App-specific password: create one at https://account.apple.com > Sign-In and Security > App-Specific Passwords"
read -rsp "App-specific password: " apppw; echo

set_secret() { printf %s "$2" | gh secret set "$1" -R "$repo"; }
set_secret APPLE_CERTIFICATE "$(base64 < "$p12" | tr -d '\n')"
set_secret APPLE_CERTIFICATE_PASSWORD "$p12pw"
set_secret APPLE_SIGNING_IDENTITY "$identity"
set_secret APPLE_TEAM_ID "$team"
set_secret APPLE_ID "$appleid"
set_secret APPLE_PASSWORD "$apppw"
rm -f "$p12"
echo "Done: the six APPLE_* secrets are set on $repo, and the .p12 file is deleted."
