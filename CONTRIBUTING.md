# Contributing to Inkwell

Thanks for helping! Bug reports, ideas, and pull requests are all welcome.

## Before you open a pull request

```sh
cargo fmt
cargo clippy --workspace --all-targets
cargo test --workspace
```

CI runs the same three commands.

## Code style

- **Simple beats clever.** Code is read far more often than it is written.
- **Reuse or extend existing functions** before adding new ones.
- **Use descriptive names** so comments are rarely needed. Comments explain *why*, not *what*.
- **Keep GUI code out of `inkwell-core`.** If logic can be tested without a window, it belongs there, with a test.
- **Don't change existing tests** unless the behavior they check is meant to change.

## Where things live

- Editing behavior (what Enter, Backspace, Ctrl+B do): `crates/inkwell-core/src/editor.rs`
- Word wrap, pagination, caret placement: `crates/inkwell-core/src/layout.rs`
- `.docx` reading and writing: `crates/inkwell-docx/src/`
- Painting and input handling: `crates/inkwell-app/src/document_view.rs`
- Toolbar, find bar, files, shortcuts: `crates/inkwell-app/src/main.rs`
