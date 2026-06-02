#!/usr/bin/env bash
# scripts/kb/selftest_gpg.sh
#
# Proves the KB GPG-verify path (scripts/kb/verify_gpg.sh / crates/kb gpg::verify_detached) is
# REAL and not a fake: it generates a THROWAWAY keypair at runtime, signs a sample KB package,
# exports the public key as the trust-anchor keyring, then asserts verify_gpg.sh ACCEPTS the good
# signature and REJECTS a one-byte-tampered package. The expected verdicts come from gpg itself
# (task no-mock constraint) -- no answer is invented.
#
# Exit codes: 0 self-test passed (accept-good + reject-tampered both hold); 2 gpg/agent seam
# (key generation could not run on this host -> skipped, not failed); 1 self-test FAILED.
# No Emoji.

set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
VERIFY="$HERE/verify_gpg.sh"

if ! command -v gpg >/dev/null 2>&1; then
  echo "SKIP: gpg not installed (deploy/00 0.7 seam)." >&2
  exit 2
fi

WORK="$(mktemp -d)"
export GNUPGHOME="$WORK/gnupg"
mkdir -p "$GNUPGHOME"
chmod 700 "$GNUPGHOME" 2>/dev/null || true
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

cat > "$WORK/keyparams" <<'EOF'
%no-protection
Key-Type: eddsa
Key-Curve: ed25519
Name-Real: Stuchka KB Selftest
Name-Email: kb-selftest@stuchka.invalid
Expire-Date: 0
%commit
EOF

if ! gpg --batch --gen-key "$WORK/keyparams" >/dev/null 2>"$WORK/gen.err"; then
  echo "SKIP: gpg key generation unavailable on this host (agent seam):" >&2
  sed 's/^/  /' "$WORK/gen.err" >&2
  exit 2
fi

PKG="$WORK/full.tar.zst"
SIG="$PKG.asc"
KEYRING="$WORK/stuchka-kb-pubkey.asc"
printf 'PRETEND-ZSTD-KB-FULL-PAYLOAD' > "$PKG"
gpg --batch --yes --armor --detach-sign --output "$SIG" "$PKG" >/dev/null 2>&1
gpg --batch --armor --output "$KEYRING" --export >/dev/null 2>&1

FAIL=0

echo "[selftest] case 1: good signature must be ACCEPTED"
if bash "$VERIFY" --keyring "$KEYRING" --package "$PKG" --signature "$SIG"; then
  echo "  PASS"
else
  echo "  FAIL: good signature was rejected"; FAIL=1
fi

echo "[selftest] case 2: tampered package must be REJECTED"
printf 'PRETEND-ZSTD-KB-FULL-PAYLOAX' > "$PKG"   # flip one byte
if bash "$VERIFY" --keyring "$KEYRING" --package "$PKG" --signature "$SIG"; then
  echo "  FAIL: tampered package was accepted"; FAIL=1
else
  echo "  PASS (rejected as expected)"
fi

if [ "$FAIL" -eq 0 ]; then
  echo "[selftest] KB GPG-verify path OK (accept-good + reject-tampered)."
  exit 0
else
  echo "[selftest] KB GPG-verify path FAILED."
  exit 1
fi
