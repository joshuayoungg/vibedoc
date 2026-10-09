# Contributing to VibeDoc

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
- **Keep GUI code out of `vibedoc-core`.** If logic can be tested without a window, it belongs there, with a test.
- **Don't change existing tests** unless the behavior they check is meant to change.

## Where things live

- Editing behavior (what Enter, Backspace, Ctrl+B do): `crates/vibedoc-core/src/editor.rs`
- Word wrap, pagination, caret placement: `crates/vibedoc-core/src/layout.rs`
- `.docx` reading and writing: `crates/vibedoc-docx/src/`
- PDF export: `crates/vibedoc-pdf/src/lib.rs`
- Diffing and undoing the assistant's changes: `crates/vibedoc-core/src/changes.rs`
- Painting the pages and turning input into edits: `crates/vibedoc-app/src/document/`
- Tabs, title, File menu, ribbon, find bar, status bar, files, autosave, shortcuts, preferences: `crates/vibedoc-app/src/app/`
- The ribbon's groups and pickers: `crates/vibedoc-app/src/app/toolbar/`
- Library sidebar (folders on disk): `crates/vibedoc-app/src/library/`
- Assistant panel, review cards, Ctrl+K bar, settings: `crates/vibedoc-app/src/assistant/`
- Colors (light and dark), fonts, icons, and shared widgets: `crates/vibedoc-app/src/ui/`
- Bundled fonts, one folder per typeface with its license: `crates/vibedoc-app/assets/fonts/`
- What the assistant is allowed to do: `crates/vibedoc-ai/src/tools.rs`
- The assistant's prompt and request loop: `crates/vibedoc-ai/src/agent.rs`
- AI providers: `crates/vibedoc-ai/src/anthropic.rs`, `openai.rs`, `images.rs`

## Adding a tool for the assistant

1. Add a `ToolSpec` to `tool_specs()` in `tools.rs`, with a clear description and JSON schema.
2. Handle it in `Workspace::run`. Return a short result the model can read, or an error it can correct.
3. If it moves paragraphs around, keep the user's selection in place with `move_selection`.
4. Add a test in `crates/vibedoc-ai/tests/agent.rs` using `ScriptedModel`. No API key needed.
