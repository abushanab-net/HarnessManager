#!/usr/bin/env bash
# Sets the six GitHub secrets the release uses to sign and notarize the macOS app with a Developer ID.
# Works on any computer with bash, openssl and gh (Git Bash on Windows too):
#   bash scripts/macos-signing-secrets.sh request          a new key, and a certificate request to upload
#                                                          at developer.apple.com (Developer ID Application)
#   bash scripts/macos-signing-secrets.sh finish <x.cer>   with the certificate Apple issued for that request
#   bash scripts/macos-signing-secrets.sh p12 <x.p12>      or with a certificate exported from a Mac keychain
# The key stays in ~/.harness-signing until finish, then is deleted. Passwords are read without echo and
# passed to gh and openssl on stdin or in the environment, never on a command line.
set -euo pipefail
repo="${REPO:-abushanab-net/HarnessManager}"
dir="$HOME/.harness-signing"
csr="$HOME/Downloads/DeveloperID.certSigningRequest"
export MSYS2_ARG_CONV_EXCL="/CN" # Git Bash would turn "/CN=..." into a Windows path

need_gh() {
  command -v gh >/dev/null || { echo "Install the GitHub CLI first."; exit 1; }
  gh auth status >/dev/null 2>&1 || gh auth login
}

# identity and team come from the certificate itself
read_cert() { # <pem file>
  local subject
  subject=$(openssl x509 -in "$1" -noout -subject -nameopt multiline)
  identity=$(sed -n 's/^ *commonName *= *//p' <<<"$subject")
  team=$(sed -n 's/^ *organizationalUnitName *= *//p' <<<"$subject")
  [[ "$identity" == "Developer ID Application:"* ]] || { echo "This is \"$identity\", not a Developer ID Application certificate."; exit 1; }
  [[ "$team" =~ ^[A-Z0-9]{10}$ ]] || { echo "No team ID in the certificate."; exit 1; }
  echo "Certificate: $identity"
}

set_secrets() { # <p12 file>; uses $P12PW, $identity, $team
  local appleid apppw
  read -rp "Apple account email: " appleid
  echo "App-specific password: https://account.apple.com > Sign-In and Security > App-Specific Passwords"
  read -rsp "App-specific password: " apppw; echo
  set_secret() { printf %s "$2" | gh secret set "$1" -R "$repo"; }
  set_secret APPLE_CERTIFICATE "$(base64 < "$1" | tr -d '\n')"
  set_secret APPLE_CERTIFICATE_PASSWORD "$P12PW"
  set_secret APPLE_SIGNING_IDENTITY "$identity"
  set_secret APPLE_TEAM_ID "$team"
  set_secret APPLE_ID "$appleid"
  set_secret APPLE_PASSWORD "$apppw"
  echo "Done: the six APPLE_* secrets are set on $repo."
}

case "${1:-}" in
  request)
    mkdir -p "$dir" "$(dirname "$csr")"; chmod 700 "$dir"
    if [ -f "$dir/key.pem" ]; then echo "Using the key from an earlier request in $dir."
    else (umask 077; openssl genrsa -out "$dir/key.pem" 2048 2>/dev/null); fi
    printf '[req]\ndistinguished_name=dn\n[dn]\n' > "$dir/req.cnf"
    openssl req -new -config "$dir/req.cnf" -key "$dir/key.pem" -subj "/CN=Harness Manager Developer ID/C=US" -out "$csr"
    echo "Certificate request: $csr"
    echo "Upload it at developer.apple.com > Certificates > + > Developer ID Application, download the"
    echo "certificate, then run: bash $0 finish <the downloaded .cer>"
    ;;
  finish)
    cer="${2:?usage: $0 finish <developerID_application.cer>}"
    test -f "$dir/key.pem" || { echo "No key from \"$0 request\" in $dir."; exit 1; }
    need_gh
    openssl x509 -inform DER -in "$cer" -out "$dir/cert.pem" 2>/dev/null || openssl x509 -in "$cer" -out "$dir/cert.pem"
    [ "$(openssl x509 -in "$dir/cert.pem" -noout -pubkey)" = "$(openssl pkey -in "$dir/key.pem" -pubout)" ] \
      || { echo "This certificate was not issued for the key in $dir."; exit 1; }
    read_cert "$dir/cert.pem"
    # Apple's intermediate goes in too, so codesign on the runner can build the chain
    curl -fsSL https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer | openssl x509 -inform DER -out "$dir/ca.pem"
    P12PW=$(openssl rand -hex 24); export P12PW
    # 3DES and SHA-1 MAC: the format every macOS version's security import reads
    openssl pkcs12 -export -inkey "$dir/key.pem" -in "$dir/cert.pem" -certfile "$dir/ca.pem" -name "$identity" \
      -certpbe PBE-SHA1-3DES -keypbe PBE-SHA1-3DES -macalg sha1 -passout env:P12PW -out "$dir/id.p12"
    set_secrets "$dir/id.p12"
    rm -rf "$dir"
    echo "The key is deleted from this computer; it now lives only in the GitHub secret."
    ;;
  p12)
    p12="${2:?usage: $0 p12 <DeveloperID.p12>}"
    need_gh
    read -rsp "Password of the .p12 file: " P12PW; echo; export P12PW
    tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
    # -legacy: OpenSSL 3 needs it for the older ciphers some Keychain exports use
    { openssl pkcs12 -in "$p12" -nokeys -clcerts -passin env:P12PW 2>/dev/null || openssl pkcs12 -legacy -in "$p12" -nokeys -clcerts -passin env:P12PW 2>/dev/null; } \
      | openssl x509 -out "$tmp/cert.pem" 2>/dev/null || { echo "Could not open the .p12: wrong password?"; exit 1; }
    read_cert "$tmp/cert.pem"
    set_secrets "$p12"
    rm -f "$p12"
    ;;
  *) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
