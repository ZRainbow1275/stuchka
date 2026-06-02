# Stučka GPG trust anchors (deploy/02 §2.2.3 + deploy/04 §4.8)

This directory is the **four-fold public-key anchoring** source-of-truth for the in-repo
copies of the project's GPG public keys. Two independent keys are used:

| File | Role | Rotation | Spec |
|------|------|----------|------|
| `stuchka-pubkey.asc` | Application **upgrade-package** verification (master key) | 5 years, 6-month dual-sign overlap | deploy/02 §2.2.3 |
| `stuchka-kb-pubkey.asc` | **Knowledge-base** distribution verification (independent subkey) | yearly | deploy/04 §4.4.3 / §4.8 |

The installer (`installer/setup.iss`) writes both files to
`%ProgramData%\Stuchka\trust\` on first install. The client (`stuchka-core.exe`,
`crates/kb`) verifies every fetched KB package against `stuchka-kb-pubkey.asc`, and the
upgrader verifies every update package against `stuchka-pubkey.asc`.

## DOCUMENTED SEAM: the real private keys

The **private** signing keys are a genuine external prerequisite that is intentionally NOT
in this repository (committing a private signing key would void the whole trust model). They
are generated **once** against the L0-01 legal subject and held offline; CI signs releases via
the `GPG_PRIVATE_KEY` / `GPG_PASSPHRASE` encrypted secrets (see `.github/workflows/release.yml`).

The `.asc` files checked in here are **public-key placeholders** clearly marked as seams. To
replace them with the real public keys once the project keypair exists:

```
gpg --armor --export <upgrade-key-id>  > keys/stuchka-pubkey.asc
gpg --armor --export <kb-subkey-id>    > keys/stuchka-kb-pubkey.asc
cp keys/*.asc installer/redist/
```

## The four-fold anchoring (so tampering with any one copy is detectable)

1. Installer-embedded copy -> `%ProgramData%\Stuchka\trust\` on first install.
2. This repo: `github.com/stuchka/stuchka/keys/` (CI hashes this dir on every push to main).
3. Domestic mirror: `gitee.com/stuchka/stuchka/raw/main/keys/`.
4. README first-screen fingerprint line (manual comparison value).

## Verifying the GPG path end-to-end without the real key (genuine, not faked)

`scripts/kb/verify_gpg.sh` and the `crates/kb` `gpg::verify_detached` function run the REAL
`gpg --verify` against whatever keyring + signature you give them. The repo test
`scripts/kb/selftest_gpg.sh` generates a throwaway keypair at runtime, signs a sample file, and
proves the verify path accepts a good signature and rejects a tampered one. No expected outcome
is fabricated -- the verdict comes from gpg itself.
