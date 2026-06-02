# KB distribution workflows (for the separate `stuchka-kb` repo)

These workflow files belong to the **independent** knowledge-base repository
`github.com/stuchka/stuchka-kb` (deploy/04 §4.2), NOT to this application repo. They are kept
here as committed reference so the KB distribution chain is reviewable alongside the app, but they
must be copied into `stuchka-kb/.github/workflows/` to take effect.

| File | Purpose | Spec |
|------|---------|------|
| `publish.yml` | Push `content/` + manifests to GitHub Pages, then purge jsDelivr | deploy/04 §4.3.1 |
| `mirror-to-gitee.yml` | Daily + on-push mirror of the KB repo to the gitee fallback | deploy/04 §4.6.2 |

They are placed under `docs/` (not `.github/workflows/`) on purpose: dropping a KB-repo workflow
into the application repo's workflow dir would make GitHub Actions try to run it here against a
non-existent `content/` tree. The GPG release-signing workflow (`sign-release.yml` in deploy/04
§4.2) reuses the same `GPG_PRIVATE_KEY` seam documented in `keys/README.md`.

The client-side GPG verify of fetched KB packages is implemented for real in this repo:
`backend/crates/kb/src/gpg.rs` (`GpgVerifier::verify_detached`) and `scripts/kb/verify_gpg.sh`,
proven end-to-end by `scripts/kb/selftest_gpg.sh`.
