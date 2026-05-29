# Contribution Guidelines

Thank you for your interest in contributing to `wayback-impersonator`! To maintain codebase quality and speed up development, we follow strict development processes.

## Trunk-Based Development (TBD)

This repository follows **Trunk-Based Development** (TBD). The key principles are:

1. **Trunk (Trunk/Main)**: The `main` branch is the trunk. It is always release-ready.
2. **Short-Lived Branches**: Features, improvements, or bug fixes are developed on short-lived branches (usually lasting less than a few days).
3. **Frequent Merges**: Merge changes back to `main` as frequently as possible. Avoid long-running feature branches.
4. **Code Quality**: Ensure all builds pass locally before merging.

## Conventional Commits

We enforce the **Conventional Commits** specification for all commit messages. This helps automate release generation and changelogs.

Format:
```text
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### Common Types

- **`feat`**: A new feature for the user (e.g. `feat(download): add concurrent thread pool`).
- **`fix`**: A bug fix (e.g. `fix(decompress): handle corrupted gzip header`).
- **`docs`**: Documentation updates (e.g. `docs: add installation instructions`).
- **`style`**: Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc.).
- **`refactor`**: A code change that neither fixes a bug nor adds a feature.
- **`perf`**: A code change that improves performance.
- **`test`**: Adding missing tests or correcting existing tests.
- **`chore`**: Maintenance tasks or build configuration updates.

### Rules

- Use the imperative mood in the description (e.g. "add support for..." not "added support for...").
- Keep the first line short (under 72 characters).

## Development Workflow

1. **Clone the repository**:
   ```bash
   git clone https://github.com/ajsb85/wayback-impersonator.git
   ```
2. **Create a short-lived branch**:
   ```bash
   git checkout -b feat/my-new-feature
   ```
3. **Make your changes**:
   - Write clean Rust code.
   - Verify it compiles: `cargo build`
   - Run tests: `cargo test` (if applicable)
4. **Commit using Conventional Commit messages**:
   - Make sure your commits are GPG-signed.
5. **Push your branch and merge to `main`** via Pull Request.

## GPG Commit and Tag Signing

To ensure codebase integrity, both commits and release tags must be signed using GPG.

### 1. Configure Local Git Signing
Configure Git with your signing key:
```bash
git config --local user.name "Alexander Salas Bastidas"
git config --local user.email "ajsb85@firechip.dev"
git config --local user.signingkey "YOUR_KEY_ID"
git config --local commit.gpgsign true
```

### 2. Sign Tag Releases
When releasing a new version, tag the commit using a signed tag:
```bash
git tag -s v0.1.1 -m "Release v0.1.1"
```

If you need to enter the GPG passphrase headlessly or programmatically during git commands, you can configure a helper script:
```bash
# Example gpg_sign.sh script
#!/bin/bash
gpg --pinentry-mode loopback --passphrase "YOUR_PASSPHRASE" "$@"
```
Then configure Git to use it:
```bash
git config --local gpg.program "/path/to/gpg_sign.sh"
```
*(Ensure to delete or secure the script after operations so passphrase is not exposed)*

## CI/CD and Release Automation

We use GitHub Actions to automate the build, Debian packaging, GPG signing, and hosting pipeline:

1. **Trigger**: Pushing a version tag matching `v*` (e.g., `v0.1.1`) to GitHub triggers the release workflow `.github/workflows/deploy.yml`.
2. **Dependencies & Build**: The runner installs `libcurl-impersonate` build dependencies, compiles the binary in `--release` mode, and runs `cargo deb` to generate the `.deb` package.
3. **APT Repository Generation**: The workflow prepares the package metadata files for two flat repository formats:
   - Root-level (`/` base directory)
   - Subdirectory-level (`/amd64/` subdirectory)
4. **Signing**: The workflow imports the release GPG private key from repository secrets and signs the repository index files (`Release` -> `InRelease` and `Release.gpg`) using GPG.
5. **Hosting**: The generated APT repository structure (including packages, metadata, and GPG public keys `archive-key.gpg`/`archive-key.asc`) is deployed to the `gh-pages` branch, making it live on GitHub Pages.
