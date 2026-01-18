# Development Scripts

This directory contains scripts to help with development workflow.

## install-hooks.sh

Installs Git hooks for the Tarbox project.

### What it does

The script installs a pre-commit hook that automatically runs:
1. `cargo fmt --all -- --check` - Ensures code is properly formatted
2. `cargo clippy --all-targets --all-features -- -D warnings` - Checks for linting issues

### Usage

After cloning the repository, run:

```bash
./scripts/install-hooks.sh
```

### Bypassing hooks

If you need to bypass the pre-commit checks (not recommended):

```bash
git commit --no-verify
```

### Why use hooks?

- **Consistency**: Ensures all commits follow the project's code style
- **Early detection**: Catches formatting and linting issues before CI
- **Saves time**: Prevents failed CI builds due to formatting/linting issues
- **Team alignment**: Everyone follows the same standards automatically

### Hook behavior

When you run `git commit`, the hook will:
- ✅ Allow commit if all checks pass
- ❌ Block commit if formatting issues are found (run `cargo fmt --all` to fix)
- ❌ Block commit if clippy warnings are found (fix the warnings manually)

The hook runs quickly (usually < 5 seconds) and provides clear feedback.
