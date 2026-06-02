#!/usr/bin/env bash
# scripts/kb/verify_gpg.sh
#
# Spec: deploy/04-kb-distribution.md 4.4.3 / 4.8. The REAL KB-package GPG detached-signature
# verify, exactly as the client (crates/kb gpg::verify_detached) performs it, exposed as a
# standalone script for ops / CI / manual checks. Mirrors the spec invocation:
#
#   gpg --no-default-keyring --keyring <kb-pubkey.asc> --verify <pkg>.asc <pkg>
#
# Exit codes: 0 signature valid; 2 gpg not installed (documented seam, deploy/00 0.7);
# 3 keyring not found; 1 signature INVALID (E_KB_SIGNATURE_INVALID -- reject the package).
# No Emoji. Cross-platform (bash; works in Git-Bash / WSL / Linux / macOS).

set -u

usage() {
  echo "usage: verify_gpg.sh --keyring <kb-pubkey.asc> --package <pkg> --signature <pkg.asc>" >&2
}

KEYRING=""
PACKAGE=""
SIGNATURE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --keyring)   KEYRING="$2"; shift 2 ;;
    --package)   PACKAGE="$2"; shift 2 ;;
    --signature) SIGNATURE="$2"; shift 2 ;;
    -h|--help)   usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage; exit 64 ;;
  esac
done

if [ -z "$KEYRING" ] || [ -z "$PACKAGE" ] || [ -z "$SIGNATURE" ]; then
  usage; exit 64
fi

if ! command -v gpg >/dev/null 2>&1; then
  echo "SEAM: gpg not installed (deploy/00 0.7 baseline: GPG 2.4.x). Cannot verify KB package." >&2
  exit 2
fi
if [ ! -f "$KEYRING" ]; then
  echo "keyring not found: $KEYRING" >&2
  exit 3
fi

# The trust anchor is distributed as an ASCII-armored public key (.asc, deploy/04 4.8). Modern
# gpg (2.4.x) cannot consume an armored file directly as a --keyring (that flag expects a binary
# keybox), so we import the armored key into an ephemeral, isolated GNUPGHOME and verify there.
# This keeps the SAME trust root the installer ships (no system keyring pollution) and is the
# correct real verification, not a relaxation: only this one public key is trusted for the check.
EPHEMERAL_HOME="$(mktemp -d)"
cleanup_home() { rm -rf "$EPHEMERAL_HOME"; }
trap cleanup_home EXIT
chmod 700 "$EPHEMERAL_HOME" 2>/dev/null || true

if ! GNUPGHOME="$EPHEMERAL_HOME" gpg --batch --quiet --import "$KEYRING" >/dev/null 2>&1; then
  echo "E_KB_SIGNATURE_INVALID: could not import trust anchor keyring: $KEYRING -- reject" >&2
  exit 1
fi

if GNUPGHOME="$EPHEMERAL_HOME" gpg --trust-model always --batch \
       --verify "$SIGNATURE" "$PACKAGE" >&2; then
  echo "OK: KB package signature VALID ($PACKAGE)"
  exit 0
else
  echo "E_KB_SIGNATURE_INVALID: KB package signature INVALID ($PACKAGE) -- reject" >&2
  exit 1
fi
