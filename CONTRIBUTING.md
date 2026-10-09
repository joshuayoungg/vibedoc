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
- Assistant panel, Ctrl+K bar, settings: `crates/inkwell-app/src/assistant.rs`
- What the assistant is allowed to do: `crates/inkwell-ai/src/tools.rs`
- The assistant's prompt and request loop: `crates/inkwell-ai/src/agent.rs`
- AI providers: `crates/inkwell-ai/src/anthropic.rs`, `openai.rs`, `images.rs`

## Adding a tool for the assistant

1. Add a `ToolSpec` to `tool_specs()` in `tools.rs`, with a clear description and JSON schema.
2. Handle it in `Workspace::run`. Return a short result the model can read, or an error it can correct.
3. If it moves paragraphs around, keep the user's selection in place with `move_selection`.
4. Add a test in `crates/inkwell-ai/tests/agent.rs` using `ScriptedModel`. No API key needed.
