# Contributing to Tarbox

Thank you for your interest in contributing to Tarbox! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Coding Principles](#coding-principles)
- [Development Workflow](#development-workflow)
- [Testing Requirements](#testing-requirements)
- [Submitting Changes](#submitting-changes)
- [Documentation](#documentation)
- [Community](#community)

## Code of Conduct

This project adheres to a [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code. Please report unacceptable behavior to the project maintainers.

## Getting Started

### Prerequisites

Before you begin, ensure you have the following installed:

- **Rust**: 1.92+ (Edition 2024)
- **PostgreSQL**: 14+
- **FUSE**: libfuse3 (Linux) or macFUSE (macOS)
- **Git**: For version control

### Finding Issues

Good places to start:

- Look for issues labeled [`good first issue`](https://github.com/yourusername/tarbox/labels/good%20first%20issue)
- Check issues labeled [`help wanted`](https://github.com/yourusername/tarbox/labels/help%20wanted)
- Review the [task/](task/) directory for planned features
- Ask in GitHub Discussions if you want to work on something

## Development Setup

### 1. Fork and Clone

```bash
# Fork the repository on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/tarbox.git
cd tarbox

# Add upstream remote
git remote add upstream https://github.com/yourusername/tarbox.git
```

### 2. Install Dependencies

```bash
# Install Rust toolchain (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install additional tools
cargo install cargo-tarpaulin  # For code coverage
cargo install cargo-audit      # For security audits
cargo install cargo-deny       # For license checking
```

### 3. Build the Project

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run tests to verify setup
cargo test
```

### 4. Setup PostgreSQL

```bash
# Create database
createdb tarbox_dev

# Initialize schema (once implemented)
cargo run -- init --database-url postgresql://localhost/tarbox_dev
```

## Coding Principles

This project follows the programming philosophies of **Linus Torvalds** and **John Carmack**:

### Linus Torvalds Principles

- **Simple and Direct**: Code should be simple and clear, avoid over-abstraction
- **Pragmatism**: Solve real problems, don't pursue perfectionism
- **Readability First**: Code is for humans to read, then for machines to execute
- **Avoid Over-design**: Don't design for future requirements, focus on current problems
- **Fail Fast**: Let errors surface early, don't hide problems

### John Carmack Principles

- **Functional Thinking**: Use pure functions when possible, reduce mutable state
- **Data-Oriented Design**: Think about data structures first, organize code around data
- **Performance Awareness**: Understand underlying implementation, avoid unnecessary overhead
- **Small Functions**: Functions should be short and focused, easy to understand and test
- **Explicit over Implicit**: Express intent clearly, avoid magic and hidden behavior

### Practical Guidelines

#### Error Handling
- Use `anyhow::Result` for error handling
- Don't define complex error type hierarchies
- Let errors bubble up naturally
- Use context (`.context()`) to add useful information

```rust
// Good
fn read_config(path: &Path) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)
        .context("Failed to read config file")?;
    let config = toml::from_str(&content)
        .context("Failed to parse config")?;
    Ok(config)
}

// Avoid
enum ConfigError {
    IoError(std::io::Error),
    ParseError(toml::de::Error),
}
```

#### Design Patterns
- Avoid traits unless you need polymorphism
- Prefer simple data structures (struct, enum)
- Keep functions short with single responsibility
- Avoid premature optimization, but be performance-aware

#### Comments
- Explain "why", not "what"
- Document non-obvious behavior
- Use doc comments for public APIs

```rust
// Good - explains why
// We cache this because path resolution is expensive
let cached = self.path_cache.get(path);

// Avoid - states the obvious
// Get path from cache
let cached = self.path_cache.get(path);
```

#### Code Style
- Use `cargo fmt` for consistent formatting
- Follow Rust naming conventions
- Keep line length reasonable (~100 chars)
- Group related functionality in modules

## Development Workflow

### 1. Create a Branch

```bash
# Update your local main
git checkout main
git pull upstream main

# Create a feature branch
git checkout -b feature/your-feature-name

# Or for bug fixes
git checkout -b fix/bug-description
```

### 2. Make Changes

- Write clean, well-documented code
- Follow the coding principles above
- Add tests for new functionality
- Update documentation as needed

### 3. Test Your Changes

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Check code coverage (must be >80%)
cargo tarpaulin --out Html

# Format code
cargo fmt --all

# Run linter
cargo clippy --all-targets --all-features -- -D warnings

# Pre-commit check (all in one)
cargo fmt --all && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo test
```

### 4. Commit Changes

Write clear, descriptive commit messages:

```bash
# Good commit messages
git commit -m "Add layer switching functionality"
git commit -m "Fix path resolution for nested directories"
git commit -m "Improve text diff performance by 30%"

# Avoid vague messages
git commit -m "fix bug"
git commit -m "update code"
```

Commit message format:
```
Short summary (50 chars or less)

More detailed explanation if needed. Wrap at 72 characters.
Explain the problem this commit solves and why you chose
this particular solution.

Fixes #123
```

### 5. Push and Create Pull Request

```bash
# Push your branch
git push origin feature/your-feature-name

# Create Pull Request on GitHub
```

## Testing Requirements

**All code must maintain test coverage > 80%.** This is a project-wide requirement.

### Types of Tests

#### Unit Tests
Test individual functions and modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_resolution() {
        let resolver = PathResolver::new();
        let result = resolver.resolve("/path/to/file");
        assert!(result.is_ok());
    }
}
```

#### Integration Tests
Test complete workflows in `tests/` directory:

```rust
// tests/layer_operations.rs
#[tokio::test]
async fn test_layer_checkpoint_and_switch() {
    // Setup
    let db = setup_test_db().await;
    let tenant = create_test_tenant(&db).await;
    
    // Test checkpoint creation
    let layer_id = create_checkpoint(&db, tenant.id).await.unwrap();
    
    // Test layer switching
    switch_layer(&db, tenant.id, layer_id).await.unwrap();
    
    // Verify
    let current = get_current_layer(&db, tenant.id).await.unwrap();
    assert_eq!(current, layer_id);
}
```

#### Benchmark Tests
Performance tests in `benches/` directory:

```rust
// benches/path_resolution.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_path_resolution(c: &mut Criterion) {
    c.bench_function("resolve_path", |b| {
        b.iter(|| resolve_path(black_box("/path/to/file")))
    });
}

criterion_group!(benches, bench_path_resolution);
criterion_main!(benches);
```

### Running Tests

```bash
# All tests
cargo test

# With output
cargo test -- --nocapture

# Specific test
cargo test test_name

# Integration tests only
cargo test --test '*'

# Benchmarks
cargo bench

# Coverage report
cargo tarpaulin --out Html --output-dir coverage
```

## Submitting Changes

### Pull Request Guidelines

1. **Title**: Clear and descriptive
   - Good: "Add native mount support for read-only directories"
   - Avoid: "Update code"

2. **Description**: Include:
   - What changes were made
   - Why the changes were necessary
   - How to test the changes
   - Related issues (e.g., "Fixes #123")

3. **Checklist** (include in PR description):
   ```markdown
   - [ ] Tests added/updated
   - [ ] Documentation updated
   - [ ] Code coverage >80%
   - [ ] `cargo fmt` applied
   - [ ] `cargo clippy` passes with no warnings
   - [ ] All tests pass
   - [ ] CHANGELOG.md updated (for significant changes)
   ```

4. **Size**: Keep PRs focused and reasonably sized
   - Large features should be split into multiple PRs
   - Each PR should do one thing well

### Review Process

1. Automated checks run on your PR (CI/CD)
2. Maintainers review your code
3. Address any feedback
4. Once approved, your PR will be merged

### After Your PR is Merged

1. Delete your feature branch
2. Update your local main branch
3. Celebrate your contribution!

```bash
git checkout main
git pull upstream main
git branch -d feature/your-feature-name
```

## Documentation

### Code Documentation

Use Rust doc comments for public APIs:

```rust
/// Resolves a path to an inode within the given tenant context.
///
/// # Arguments
///
/// * `tenant_id` - The tenant's unique identifier
/// * `path` - The absolute path to resolve
///
/// # Returns
///
/// Returns `Ok(InodeId)` if the path exists, or an error if:
/// - The path is invalid
/// - The path doesn't exist
/// - Permission denied
///
/// # Examples
///
/// ```
/// let inode_id = resolver.resolve_path(tenant_id, "/path/to/file")?;
/// ```
pub fn resolve_path(&self, tenant_id: TenantId, path: &str) -> anyhow::Result<InodeId> {
    // Implementation
}
```

### Architecture Documentation

When making architectural changes:

1. Update relevant specs in `spec/` directory
2. Update `CLAUDE.md` if development patterns change
3. Update README if user-facing changes
4. Add notes to `CHANGELOG.md`

### Writing Documentation

- Use clear, concise language
- Include examples where helpful
- Keep documentation up-to-date with code
- Consider both beginners and advanced users

## Community

### Getting Help

- **GitHub Discussions**: Ask questions and share ideas
- **GitHub Issues**: Report bugs and request features
- **Documentation**: Check [docs/](docs/) and [spec/](spec/)

### Communication Guidelines

- Be respectful and inclusive
- Provide context in questions
- Search existing issues before creating new ones
- Use clear, descriptive titles
- Be patient with responses

### Recognition

Contributors will be:
- Listed in release notes
- Credited in CHANGELOG.md
- Mentioned in project documentation

## Additional Resources

### Project Structure

```
tarbox/
├── src/              # Source code
│   ├── storage/      # PostgreSQL operations
│   ├── fs/           # Filesystem core
│   ├── fuse/         # FUSE interface
│   ├── layer/        # Layered filesystem
│   ├── native/       # Native mounts
│   ├── audit/        # Audit logging
│   ├── cache/        # Caching layer
│   ├── api/          # REST/gRPC APIs
│   └── k8s/          # Kubernetes CSI
├── spec/             # Architecture specs
├── task/             # Development tasks
├── tests/            # Integration tests
├── benches/          # Benchmarks
└── docs/             # User documentation
```

### Useful Commands

```bash
# Dependency management (NEVER edit Cargo.toml manually)
cargo add <crate>              # Add dependency
cargo add --dev <crate>        # Add dev dependency

# Security and compliance
cargo audit                    # Security audit
cargo deny check               # License checking

# Documentation
cargo doc --open               # Build and open docs

# Cleaning
cargo clean                    # Clean build artifacts
```

### Learning Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
- [FUSE Documentation](https://www.kernel.org/doc/html/latest/filesystems/fuse.html)
- [Project Specifications](spec/)

---

## Questions?

If you have questions not covered here:

1. Check [existing documentation](docs/)
2. Search [GitHub Issues](https://github.com/yourusername/tarbox/issues)
3. Ask in [GitHub Discussions](https://github.com/yourusername/tarbox/discussions)
4. Read [CLAUDE.md](CLAUDE.md) for detailed development guidelines

Thank you for contributing to Tarbox!
