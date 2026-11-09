# Code signing policy

Releases are **not code-signed yet**. This policy describes how signing works once a
certificate is in place.

## What is signed

Only binaries built from this repository's source code by its GitHub Actions
release workflow ([`.github/workflows/release.yml`](.github/workflows/release.yml))
are signed:

- `SEPA-Validator-<version>-windows-x64-setup.exe` (the installer) and the
  `SEPA Validator` program inside it
- `SEPA-Validator-<version>-windows-x64-portable.exe` (the portable program)

Releases are built only on GitHub-hosted runners from a version tag on `master`.
Every signing request is approved manually by an approver (see below) after
checking that it comes from the expected release run. Third-party libraries
compiled into the program (see [`app/THIRD-PARTY-NOTICES.txt`](app/THIRD-PARTY-NOTICES.txt))
are not signed separately.

## Team roles

| Role | Members |
|---|---|
| Committers and reviewers | [Leon Kasdorf](https://github.com/lkasdorf) |
| Approvers | [Leon Kasdorf](https://github.com/lkasdorf) |

Committers may change the source code without further review. Changes from
anyone else, such as pull requests, are reviewed by a reviewer before they are
merged. Approvers approve each signing request. All team members use
multi-factor authentication for GitHub and the signing service.

## Privacy policy

This program will not transfer any information to other networked systems
unless specifically requested by the user or the person installing or operating
it.

The program connects to the internet only when you ask it to:

- **☰ → Check for Updates** downloads the update manifest and, on your
  confirmation, the new version from this project's GitHub releases. GitHub's
  [privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement)
  applies to these downloads.
- Links you click (for example the online IBAN check next to an IBAN, or the
  schema download pages) open in your web browser.

Payment files, statements and schemas are processed locally and never uploaded.
