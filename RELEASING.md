# Releasing

Releases are built, signed and uploaded by `.github/workflows/release.yml` as a
**draft**. You smoke-test the draft and publish it; the auto-updater then picks
it up from `releases/latest/download/latest.json`.

The app version lives in one place: `"version"` in `app/src-tauri/tauri.conf.json`
(`Cargo.toml` and `package.json` have none). Versions are plain `X.Y.Z`.

## One-time setup

1. **Back up the updater signing key.** `~/.tauri/sepa-validator.key` (+ `.pub`) is
   the only key that existing installs accept. Keep a copy in your password manager
   and one offline. If it is lost, users must reinstall manually.
2. **Create the `release` environment** (Settings → Environments → New environment):
   - Required reviewers: yourself, so every signing run waits for your approval.
   - Deployment branches and tags: tags matching `v*` (plus `master` if you want dry runs).
3. **Add the key as an environment secret** (run it yourself, in Git Bash):
   ```sh
   gh secret set TAURI_SIGNING_PRIVATE_KEY --env release --repo lkasdorf/SEPA-Validator < ~/.tauri/sepa-validator.key
   ```
   The key has no password, so `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` can stay unset.
4. **Optional dry run:** Actions → Release → Run workflow (on `master`). It builds and
   signs, then keeps the installer, `.sig` and portable exe as workflow artifacts
   instead of creating a release.

## Each release

1. On a branch: `node scripts/bump-version.mjs X.Y.Z`. This sets the version and turns
   `## [Unreleased]` in `CHANGELOG.md` into `## [X.Y.Z] - <date>` (and fixes the
   compare links). Review, open a PR, merge it when CI is green.
2. Tag the merge commit on master and push the tag:
   ```sh
   git checkout master && git pull
   git tag vX.Y.Z && git push origin vX.Y.Z
   ```
3. Approve the `release` environment run. The workflow checks that the tag matches
   the version and is on master, then creates the draft release **SEPA Validator vX.Y.Z**
   with the CHANGELOG section as notes and:
   - `SEPA-Validator-X.Y.Z-windows-x64-setup.exe` (+ `.sig`)
   - `SEPA-Validator-X.Y.Z-windows-x64-portable.exe`
   - `latest.json` (updater manifest)
4. Install the draft's setup exe and run the portable exe once. Then publish the draft
   and mark it as the latest release.

## Rotating the signing key

Generate a new key with `npx tauri signer generate`. Put its public key into
`plugins.updater.pubkey` in `tauri.conf.json`, but sign that release with the **old**
key (installed apps only trust the old one). Switch the `TAURI_SIGNING_PRIVATE_KEY`
secret to the new key from the following release on.
