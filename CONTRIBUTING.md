# Contributing to Tarbox

## Development Setup

1. Install Rust 1.92+ with Edition 2024 support
2. Install PostgreSQL 14+
3. Clone the repository
4. Run `make install-deps` to install development tools

## Coding Principles

This project follows the programming philosophies of Linus Torvalds and John Carmack:

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
- Don't define complex error type systems, use `anyhow` to let errors bubble up
- Avoid over-abstraction and trait design unless polymorphism is actually needed
- Prefer simple data structures (struct, enum)
- Keep functions short with single responsibility
- Avoid premature optimization, but maintain performance awareness
- Code comments explain "why", not "what"

## Testing Requirements

All code must maintain **test coverage > 80%**. This is a project-wide requirement to ensure code quality and reliability.

- Write unit tests for all new functions
- Write integration tests for complete workflows
- Test edge cases and error conditions
- Use `cargo test` to run tests
- Consider using `cargo-tarpaulin` or similar tools to measure coverage

## Development Workflow

1. Create a feature branch
2. Make your changes following the coding principles above
3. Write tests to maintain coverage > 80%
4. Run `make check` to verify formatting, linting, and tests
5. Submit a pull request

## Running Tests

```bash
make test
```

## Code Formatting

```bash
make fmt
```

## Linting

```bash
make lint
```

## Security Audit

```bash
make audit
```
