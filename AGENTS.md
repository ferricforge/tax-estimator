## Session Initialization
- Review README.md and all documentation in the `doc/` directory.

## Code Style
- Run `cargo fmt` after changes. Maintain zero clippy warnings.
- Qualify or rename imports to resolve name conflicts.

## Error Handling
- Use `anyhow::Result` for propagation, `thiserror` for custom error types.
- Do not use `.unwrap()`; select an option that won't panic.

## Documentation
- Document all public APIs with `///` doc comments.
- Rustdoc examples must be self-contained with `assert`s; no file I/O or network access.
- Check existing tests before adding examples to avoid duplication.
- Never use meta-comments like `// Added this field` or `// <- Changed this`.

## Testing
- Unit tests in `#[cfg(test)]` modules; one scenario per test function.
- Integration tests in `tests/` directory.
- Run `cargo test` before completing tasks.
- Use `tarpaulin` for coverage, output to `coverage/`.

## Dependencies
- Use `cargo add` rather than editing Cargo.toml directly.

## Build
- Run `cargo build` after changes; fix all warnings and errors before submitting.

## Available Cargo Commands
- `cargo --list` to see the available cargo commands
- Use built-in agent tools for reading, searching, and updating files or assets, where possible.