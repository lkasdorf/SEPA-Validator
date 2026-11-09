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
   signs, then keeps the installer, `.sig`, `latest.json` and portable exe as the
   workflow artifact `release-dry-run` instead of creating a release.
5. **Code signing (SignPath):** see [Code signing with SignPath](#code-signing-with-signpath).
   Until the `SIGNPATH_ORGANIZATION_ID` variable exists, releases stay unsigned
   (only the updater signature is made).

## Each release

1. On a branch: `node scripts/bump-version.mjs X.Y.Z`. This sets the version and turns
   `## [Unreleased]` in `CHANGELOG.md` into `## [X.Y.Z] - <date>` (and fixes the
   compare links). Review, open a PR, merge it when CI is green.
2. Tag the merge commit on master and push the tag:
   ```sh
   git checkout master && git pull
   git tag vX.Y.Z && git push origin vX.Y.Z
   ```
3. Approve the `release` environment run. With code signing set up, also approve the
   two signing requests in SignPath (first the binaries, then the installer, about
   10 minutes apart); each waits up to an hour. The workflow checks that the tag
   matches the version and is on master, then creates the draft release
   **SEPA Validator vX.Y.Z** with the CHANGELOG section and a link to the code signing
   policy as notes and:
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

## Code signing with SignPath

**Status:** not active. The workflow is prepared for the free
[SignPath Foundation](https://signpath.org/) programme for open source projects, but the
application was declined in November 2026 because the project does not yet have enough
public visibility (stars, forks, external references). Reapply once it has. Until
`SIGNPATH_ORGANIZATION_ID` is set, releases stay unsigned.

With SignPath Foundation, the publisher shown by Windows is "SignPath Foundation". SignPath
requires [`CODE_SIGNING_POLICY.md`](CODE_SIGNING_POLICY.md) (what is signed, team roles,
privacy statement), the "Code signing policy" link in the README and release notes, and the
sentence "Free code signing provided by SignPath.io, certificate by SignPath Foundation"
with links on that page and in the README.

### How the workflow signs

`tauri build --no-bundle` builds `app.exe`. The workflow then:

1. Makes two copies: `portable.exe` as built, and `app.exe` with the NSIS bundle type
   patched in (`scripts/release-assets.mjs patch-nsis`; `tauri bundle` would patch it
   otherwise, which would break a signature made before).
2. Uploads both as the artifact `unsigned-binaries` and submits signing request 1
   (artifact configuration `binaries`).
3. Packs the signed `app.exe` into the installer with `tauri bundle --no-sign`.
4. Uploads the installer as `unsigned-installer` and submits signing request 2
   (artifact configuration `installer`).
5. Makes the updater signature of the signed installer (`tauri signer sign`) and
   writes `latest.json` (`scripts/release-assets.mjs manifest`).

The NSIS uninstaller and NSIS plugin DLLs inside the installer stay unsigned.

### One-time setup

1. **Apply** at <https://signpath.org/apply> with the repository URL, a short description
   and the project's download page (the GitHub releases page). Make sure your GitHub
   account uses two-factor authentication.
2. **After approval, in SignPath** (<https://app.signpath.io>):
   - Enable two-factor authentication for your SignPath account.
   - Under *Trusted Build Systems*, add **GitHub.com** and link it to the project.
   - Check that the project slug is `SEPA-Validator` and the signing policy slug is
     `release-signing`; if they differ, change `SIGNPATH_PROJECT_SLUG` /
     `SIGNPATH_POLICY_SLUG` in `release.yml`.
   - Add the two artifact configurations below (slugs `binaries` and `installer`).
   - Create a CI user with the *Submitter* role on the release signing policy and
     generate an API token for it. You stay the *Approver*.
   - Optionally install the SignPath GitHub App for the repository.
3. **In GitHub:**
   - Store the token as an environment secret (run it yourself, in Git Bash; it prompts
     for the value):
     ```sh
     gh secret set SIGNPATH_API_TOKEN --env release --repo lkasdorf/SEPA-Validator
     ```
   - Set the organization ID (SignPath → Organization → Settings) as a repository
     variable. This switches signing on:
     ```sh
     gh variable set SIGNPATH_ORGANIZATION_ID --repo lkasdorf/SEPA-Validator --body "<organization id>"
     ```
4. **Test** with a dry run (Actions → Release → Run workflow). It goes through both
   signing requests; check the signature of the files in `release-dry-run`
   (right-click → Properties → Digital Signatures).

Artifact configuration `binaries` (the uploaded artifact is a ZIP with two files):

```xml
<?xml version="1.0" encoding="utf-8"?>
<artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1">
  <parameters>
    <parameter name="version" />
  </parameters>
  <zip-file>
    <pe-file path="app.exe" product-name="SEPA Validator" product-version="${version}">
      <authenticode-sign />
    </pe-file>
    <pe-file path="portable.exe" product-name="SEPA Validator" product-version="${version}">
      <authenticode-sign />
    </pe-file>
  </zip-file>
</artifact-configuration>
```

Artifact configuration `installer`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1">
  <parameters>
    <parameter name="version" />
  </parameters>
  <zip-file>
    <pe-file path="setup.exe" product-name="SEPA Validator" product-version="${version}">
      <authenticode-sign />
    </pe-file>
  </zip-file>
</artifact-configuration>
```

The workflow passes `version` from `tauri.conf.json`, so SignPath refuses files whose
product name or version do not match. These configurations are kept here, not as
`.xml` files, because the PII guard rejects XML outside the test fixtures.

### SmartScreen

A signature does not remove the SmartScreen warning at once: Windows builds reputation
per certificate from downloads. Until then the dialog names "SignPath Foundation" as
the publisher instead of "Unknown publisher".
