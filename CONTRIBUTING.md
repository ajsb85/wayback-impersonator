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
   - Run tests if any: `cargo test`
4. **Commit with Conventional Commits format**:
   - Make sure your commits are signed with your GPG key.
5. **Merge to main** via Pull Request.
