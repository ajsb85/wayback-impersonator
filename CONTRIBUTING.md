# Contribution Guidelines

Thank you for your interest in contributing to `wayback`! This document covers
everything you need to know: workflow, commit conventions, packaging standards,
CI/CD pipeline, and how to add new content (man page, completions, etc.).

---

## Table of Contents

- [Development Workflow](#development-workflow)
- [Conventional Commits](#conventional-commits)
- [GPG Commit & Tag Signing](#gpg-commit--tag-signing)
- [Packaging & Debian Assets](#packaging--debian-assets)
  - [Man Page](#man-page)
  - [Shell Completions](#shell-completions)
  - [Copyright & Changelog](#copyright--changelog)
  - [Wiring Assets into cargo-deb](#wiring-assets-into-cargo-deb)
- [Releasing a New Version](#releasing-a-new-version)
- [CI/CD Pipeline](#cicd-pipeline)
- [Coding Standards](#coding-standards)

---

## Development Workflow

This project uses **Trunk-Based Development** (TBD):

- `main` is always release-ready and is the single source of truth.
- All work happens on **short-lived branches** (ideally merged within 1–2 days).
- Never push directly to `main` unless it is a trivial doc or packaging change.

```bash
# 1. Clone
git clone https://github.com/ajsb85/wayback-impersonator.git
cd wayback-impersonator

# 2. Create a short-lived branch
git checkout -b feat/my-feature

# 3. Develop, verify compilation
cargo build
cargo test

# 4. Commit with Conventional Commits format (see below)
git commit -S -m "feat(browser): add safari18 impersonation profile"

# 5. Push and open a Pull Request against main
git push origin feat/my-feature
```

---

## Conventional Commits

All commit messages **must** follow the
[Conventional Commits](https://www.conventionalcommits.org/) specification.

```
<type>(<scope>): <short description>

[optional body — wrap at 72 chars]

[optional footer(s)]
```

### Types

| Type | When to use |
|---|---|
| `feat` | New feature for the user |
| `fix` | Bug fix |
| `docs` | Documentation only (README, man page, etc.) |
| `style` | Whitespace, formatting — no behaviour change |
| `refactor` | Code restructuring — no feature or fix |
| `perf` | Performance improvement |
| `test` | Adding or correcting tests |
| `chore` | Build config, CI, dependency bumps |
| `packaging` | Debian assets (man page, completions, changelog) |

### Rules

- Use the **imperative mood**: "add support for…", not "added support for…".
- Keep the first line under **72 characters**.
- Reference issues in the footer: `Closes #42`, `Fixes #7`.

### Examples

```
feat(resume): add --retry-errors flag for targeted failure retry
fix(decompress): handle truncated brotli stream without panicking
docs(readme): add shell completion installation instructions
packaging(man): update wayback.1 with --retry-errors option
chore(ci): upgrade actions/checkout to v4 for Node 24 compatibility
```

---

## GPG Commit & Tag Signing

All commits and release tags must be signed with GPG to ensure codebase integrity.

### Configure local Git signing

```bash
git config --local user.name "Your Name"
git config --local user.email "you@example.com"
git config --local user.signingkey "YOUR_GPG_KEY_ID"
git config --local commit.gpgsign true
git config --local tag.gpgsign true
```

### Sign a release tag

```bash
git tag -s v0.1.9 -m "feat: release v0.1.9"
git push origin v0.1.9
```

Pushing a `v*` tag triggers the automated release workflow (see [CI/CD Pipeline](#cicd-pipeline)).

---

## Packaging & Debian Assets

All Debian packaging assets live in the [`debian/`](debian/) directory and are
declared as `cargo-deb` assets in [`Cargo.toml`](Cargo.toml). Follow these
conventions when adding or editing them.

### Man Page

File: [`debian/wayback.1`](debian/wayback.1)
Installed to: `/usr/share/man/man1/wayback.1.gz` ([Debian Policy §12.1](https://www.debian.org/doc/debian-policy/ch-docs.html#man-pages))

The man page is written in **troff/nroff** format. Key macros used:

| Macro | Purpose |
|---|---|
| `.TH` | Title header — name, section, date, version, group |
| `.SH` | Section heading (NAME, SYNOPSIS, DESCRIPTION, OPTIONS…) |
| `.TP` | Tagged paragraph — flag name + description |
| `.EX` / `.EE` | Example block (monospace, no-fill) |
| `.UR` / `.UE` | Hyperlink (terminal-safe) |
| `.MT` / `.ME` | Email address |

**Adding a new flag to the man page:**

```troff
.TP
.BR \-\-my\-flag \ \fIPATTERN\fR
Description of what the flag does.
```

**Lint before committing:**

```bash
man --warnings -E UTF-8 -l -Tutf8 debian/wayback.1 > /dev/null
```

**Render locally:**

```bash
man ./debian/wayback.1
```

> The CI workflow compresses the man page with `gzip -9 --keep debian/wayback.1`
> before `cargo deb` runs. Do **not** commit the `.gz` file; it is generated
> in the runner.

---

### Shell Completions

| File | Shell | Installed to |
|---|---|---|
| [`debian/wayback.bash-completion`](debian/wayback.bash-completion) | Bash | `/usr/share/bash-completion/completions/wayback` |
| [`debian/wayback.zsh`](debian/wayback.zsh) | Zsh | `/usr/share/zsh/vendor-completions/_wayback` |
| [`debian/wayback.fish`](debian/wayback.fish) | Fish | `/usr/share/fish/vendor_completions.d/wayback.fish` |

**When to update completions:**

- A new flag is added to `src/main.rs` — add it to all three completion files.
- A new browser profile is supported — add it to the browser profile lists in
  all three files.
- A new common MIME type is useful — add it to the MIME suggestion lists.

**Bash** — uses `_init_completion` + `case "$prev"` pattern.  
**Zsh** — uses `_arguments` with `-s -S` and `_describe` for enums.  
**Fish** — uses `complete -c wayback` with `-a` for argument completion and `-f` to
disable file completion globally.

**Test completions without installing:**

```bash
# Bash
source debian/wayback.bash-completion && wayback --[TAB]

# Zsh (in a zsh session)
fpath=(./debian $fpath) && autoload -U compinit && compinit
cp debian/wayback.zsh /tmp/_wayback && fpath=(/tmp $fpath) && compinit

# Fish
cp debian/wayback.fish ~/.config/fish/completions/ && wayback [TAB]
```

---

### Copyright & Changelog

**Copyright** — [`debian/copyright`](debian/copyright)  
Format: [DEP-5 machine-readable](https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/)  
([Debian Policy §12.5](https://www.debian.org/doc/debian-policy/ch-docs.html#copyright-information))

If you add a dependency with a different licence, add a new `Files:` stanza:

```
Files: vendor/some-lib/*
Copyright: 2024 Some Author <author@example.com>
License: Apache-2.0
```

**Changelog** — [`debian/changelog`](debian/changelog)  
Format: standard Debian changelog ([Debian Policy §12.7](https://www.debian.org/doc/debian-policy/ch-docs.html#the-changelog-files))

Add a new entry at the **top** of the file for every release:

```
wayback (X.Y.Z-1) unstable; urgency=low

  * Short description of change 1.
  * Short description of change 2.

 -- Your Name <you@example.com>  Day, DD Mon YYYY HH:MM:SS +ZZZZ
```

> The CI workflow compresses the changelog with `gzip -9 --keep debian/changelog`
> before `cargo deb` runs. Do **not** commit the `.gz` file.

---

### Wiring Assets into cargo-deb

All packaging assets are declared in [`Cargo.toml`](Cargo.toml) under
`[package.metadata.deb]`:

```toml
[package.metadata.deb]
name       = "wayback"
maintainer = "Alexander Salas Bastidas <ajsb85@firechip.dev>"
section    = "net"
priority   = "optional"
assets = [
    # ["<source>",  "<install path>",  "<permissions>"]
    ["target/release/wayback-impersonator", "/usr/bin/wayback",                                    "755"],
    ["debian/wayback.1.gz",                "/usr/share/man/man1/wayback.1.gz",                     "644"],
    ["debian/copyright",                   "/usr/share/doc/wayback/copyright",                     "644"],
    ["debian/changelog.gz",                "/usr/share/doc/wayback/changelog.gz",                  "644"],
    ["debian/wayback.bash-completion",     "/usr/share/bash-completion/completions/wayback",        "644"],
    ["debian/wayback.zsh",                 "/usr/share/zsh/vendor-completions/_wayback",            "644"],
    ["debian/wayback.fish",                "/usr/share/fish/vendor_completions.d/wayback.fish",     "644"],
]
```

Source paths are **relative to the repository root**. The `.gz` files for the man
page and changelog are produced by the workflow step
`Compress man page and changelog (Debian policy §12.1 / §12.7)` and must **not**
be committed to git.

---

## Releasing a New Version

Follow these steps in order:

1. **Bump the version** in `Cargo.toml` (`version = "X.Y.Z"`).

2. **Add a changelog entry** at the top of `debian/changelog`.

3. **Update the man page** `debian/wayback.1` date/version in the `.TH` line:
   ```troff
   .TH WAYBACK 1 "YYYY-MM-DD" "X.Y.Z" "Wayback Machine Downloader"
   ```

4. **Commit** everything:
   ```bash
   git add Cargo.toml debian/
   git commit -S -m "chore: bump to vX.Y.Z"
   git push origin main
   ```

5. **Create and push a signed tag** — this triggers the full CI/CD pipeline:
   ```bash
   git tag -a vX.Y.Z -m "chore: release vX.Y.Z"
   git push origin vX.Y.Z
   ```

6. Monitor the workflow at
   [github.com/ajsb85/wayback-impersonator/actions](https://github.com/ajsb85/wayback-impersonator/actions).

---

## CI/CD Pipeline

The workflow [`deploy.yml`](.github/workflows/deploy.yml) runs on every `v*` tag push.

```
Push v* tag
    │
    ▼
┌─────────────────────────────┐
│ Checkout code               │
├─────────────────────────────┤
│ Install Rust + cargo-deb    │
├─────────────────────────────┤
│ Install APT utilities       │
├─────────────────────────────┤
│ Install libcurl-impersonate │  (required to link the binary)
├─────────────────────────────┤
│ Compress man page &         │  gzip -9 --keep debian/wayback.1
│ changelog                   │  gzip -9 --keep debian/changelog
├─────────────────────────────┤
│ cargo build --release       │
│ cargo deb                   │  produces wayback_X.Y.Z-1_amd64.deb
├─────────────────────────────┤
│ Setup APT repo directory    │  dpkg-scanpackages, apt-ftparchive
├─────────────────────────────┤
│ Import GPG key & sign       │  InRelease, Release.gpg, .deb.asc
├─────────────────────────────┤
│ Render index.html           │  sed {{VERSION}} placeholder
├─────────────────────────────┤
│ Create GitHub Release       │  gh release create vX.Y.Z
├─────────────────────────────┤
│ Deploy to gh-pages          │  JamesIves/github-pages-deploy-action
└─────────────────────────────┘
```

**Required GitHub Secrets:**

| Secret | Description |
|---|---|
| `GPG_PRIVATE_KEY` | ASCII-armoured GPG private key used to sign the repo and `.deb` |
| `GPG_PASSPHRASE` | Passphrase for the private key |
| `GITHUB_TOKEN` | Auto-provided by GitHub Actions (no setup needed) |

---

## Coding Standards

- **Rust edition**: 2024 (set in `Cargo.toml`).
- **Error handling**: use `anyhow` for application errors; avoid `unwrap()` in
  non-test code.
- **Formatting**: run `cargo fmt` before committing.
- **Linting**: run `cargo clippy -- -D warnings` and fix all warnings.
- **No unsafe**: avoid `unsafe` blocks unless strictly necessary and documented.
- **Thread safety**: shared state must use `Arc<Mutex<T>>` or atomics.

```bash
# Full local check before pushing
cargo fmt --check
cargo clippy -- -D warnings
cargo build --release
cargo test
```
