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
- [Code Architecture](#code-architecture)
- [Testing](#testing)
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

# 3. Develop and verify
cargo build
cargo test                              # must be 0 failures
cargo clippy --all-targets -- -D warnings  # must be 0 warnings

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
test(sanitize): add edge-case for root path trimming
fix(lint): resolve clippy::collapsible_if in decompression block
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
git tag -s v1.0.0 -m "feat: release v1.0.0"
git push origin v1.0.0
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

## Code Architecture

Since `v1.0.0` the repository uses a **library + binary crate** split:

```
src/
├── lib.rs   ← wayback_impersonator (library crate)
│             All public types and functions with rustdoc comments.
│             Contains the full #[cfg(test)] unit-test suite.
│             Crate-level //! doc comment describes the overall design.
│
└── main.rs  ← wayback-impersonator (binary crate)
              Clap CLI definition + orchestration loop only.
              Uses `wayback_impersonator::*` — no business logic here.
```

**Rules:**

- **All new logic goes in `lib.rs`** and must be `pub` with a `///` doc comment.
- **`main.rs`** must stay thin — it may only contain `Cli` struct, `main()`,
  and thread-spawning glue code.
- **New public types** must derive `Debug` and, where serialised, `Serialize +
  Deserialize`.
- **Doc-tests** in `///` examples are automatically run by `cargo test` —
  keep them accurate and compiling.

---

## Testing

All tests live in the `#[cfg(test)] mod tests` block at the bottom of
[`src/lib.rs`](src/lib.rs). They run fully offline — no network required.

### Running the suite

```bash
# Run all unit tests + doc-tests
cargo test

# Run only tests matching a pattern
cargo test sanitize
cargo test journal

# Run a single test by exact name
cargo test tests::build_tasks_dedup_keeps_latest_timestamp

# Run ignored integration tests (require CDX API access)
cargo test -- --ignored
```

### Test inventory (33 total)

| Group | Count | Functions under test |
|---|---|---|
| `sanitize_filename` | 7 | Path normalisation, query strip, ext truncation, invalid URL fallback |
| `build_tasks` | 6 | Deduplication, latest-timestamp selection, MIME→extension, all-pending status |
| `DownloadStatus` | 2 | Enum equality, `Failed` reason round-trip |
| `decompress_gzip` | 4 | Roundtrip, empty payload, invalid bytes → error, magic-byte detection |
| `decompress_brotli` | 3 | Roundtrip, empty payload, invalid bytes → passthrough fallback |
| `save_journal` | 5 | JSON round-trip, valid JSON, atomic rename (no `.tmp`), `Failed` reason, empty tasks |
| Doc-tests | 6 | All `///` code examples in `lib.rs` |

### Writing a new test

```rust
#[test]
fn my_module_scenario_description() {
    // Arrange
    let input = ...;

    // Act
    let result = my_function(input);

    // Assert
    assert_eq!(result, expected);
}
```

**Rules:**

- Name tests as `module_scenario` (e.g. `sanitize_strips_query_after_extension`).
- Use `tempfile::tempdir()` for all disk I/O — never hard-code `/tmp` paths.
- Mark tests that require the live CDX API with `#[ignore]`:
  ```rust
  #[test]
  #[ignore = "requires live Wayback Machine CDX API"]
  fn integration_query_cdx_returns_records() { ... }
  ```
- Helper functions (e.g. compression fixtures) go above the tests, not inside
  individual `#[test]` functions, so they can be shared.
- Do not use `unwrap()` in assertion logic — use `expect("message")` so failures
  are meaningful.

### Linting

All code must pass clippy with zero warnings:

```bash
cargo clippy --all-targets --all-features
```

The specific lint that was fixed in `v1.0.0` (`clippy::collapsible_if`) is a
good example of what to watch for: nested `if { if let Ok(...) }` should always
be collapsed to `if condition && let Ok(d) = expr { }` in edition 2024+.

To apply automatic fixes suggested by clippy:

```bash
cargo clippy --fix --all-targets
```

### Rustdoc

All doc-comments must also be warning-free:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

---

## Releasing a New Version

Follow these steps in order:

1. **Bump the version** in `Cargo.toml` (`version = "X.Y.Z"`).

2. **Add a changelog entry** at the top of `debian/changelog`.

3. **Update the man page** `debian/wayback.1` date/version in the `.TH` line:
   ```troff
   .TH WAYBACK 1 "YYYY-MM-DD" "X.Y.Z" "Wayback Machine Downloader"
   ```

4. **Run the full quality gate locally** and fix any issues before tagging:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
   ```

5. **Commit** everything:
   ```bash
   git add Cargo.toml debian/
   git commit -S -m "chore: bump to vX.Y.Z"
   git push origin main
   ```

6. **Create and push a signed tag** — this triggers the full CI/CD pipeline:
   ```bash
   git tag -a vX.Y.Z -m "chore: release vX.Y.Z"
   git push origin vX.Y.Z
   ```

7. Monitor the workflow at
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

### Rust edition

**2024** (set in `Cargo.toml`). Use edition-2024 features where appropriate,
including `let` chains in `if` conditions (`if cond && let Ok(x) = expr { }`).

### Error handling

- Use [`anyhow`](https://docs.rs/anyhow) for all application-level errors.
- Avoid `unwrap()` in non-test code — use `?` or `.context("message")`.
- In tests, prefer `.expect("descriptive message")` over bare `.unwrap()`.

### Code documentation

- Every `pub` item in `src/lib.rs` **must** have a `///` doc comment.
- Module-level docs use `//!` at the top of the file.
- Doc-tests (`///` code blocks) must compile and pass — they run with `cargo test`.
- Run `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` to catch broken links and
  missing docs.

### Formatting & linting

```bash
cargo fmt                              # auto-format (run before committing)
cargo fmt --check                      # CI-style check (fails if unformatted)
cargo clippy --all-targets -- -D warnings  # 0 warnings enforced
```

Common clippy lints to watch for:

| Lint | Rule |
|---|---|
| `clippy::collapsible_if` | Collapse `if { if let }` → `if cond && let Ok(x) = expr` |
| `clippy::needless_pass_by_value` | Prefer `&str` over `String` for read-only args |
| `clippy::clone_on_ref_ptr` | Prefer `Arc::clone(&x)` over `x.clone()` for `Arc<T>` |
| `clippy::unwrap_used` | Avoid `.unwrap()` — use `?` or `.expect()` |

### Thread safety

- Shared mutable state must use `Arc<Mutex<T>>`.
- Atomic counters/indices use `Arc<AtomicUsize>` with `Ordering::Relaxed`
  for task-dispatch and stronger orderings only when synchronisation is required.

### No unsafe

Avoid `unsafe` blocks. If absolutely required, document with a `// SAFETY:`
comment explaining every invariant that makes the code sound.

### Full pre-push gate

Run this before every `git push` to `main` or before tagging a release:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo build --release
```

All five commands must exit with code `0`.
