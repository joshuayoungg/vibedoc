# Inkwell

A small, fast word processor written in Rust, with an AI assistant that edits your document for you. It opens and saves real Word (`.docx`) files.

![Inkwell after asking the assistant to add a grocery list and a picture](docs/screenshot.png)

## Features

- Paginated "print layout" view: US Letter pages, one-inch margins, word wrap
- Bold, italic, underline
- Headings 1–3, bulleted lists, numbered lists
- Open and save `.docx` files that Word and LibreOffice read correctly
- Undo / redo (typing undoes in bursts, like Word)
- Find with highlighted matches
- Copy, cut, paste, double-click to select a word, zoom
- Images, including in saved `.docx` files
- **An AI assistant** you can ask in plain English (side panel, or Ctrl+K anywhere)

## The assistant

Ask for what you want, and the assistant makes the edit:

- "Put these notes into a bulleted list"
- "Make the selected text bold"
- "Draw a watercolor fox and put it here"
- "Add a title and a short intro, then make the second paragraph a heading"

Everything it does in one request is **one undo step**, so Ctrl+Z takes it all back.

### Setup

By default the assistant uses Claude for writing and Gemini for pictures. Set your keys as environment variables:

```sh
export ANTHROPIC_API_KEY=...   # writing and editing
export GEMINI_API_KEY=...      # pictures
```

Or click **Settings** in the assistant panel and paste them in.

### Bring your own model

Any model works, as long as it supports tool calling. In **Settings** (or `config.toml`, whose path is shown there), pick:

- **Claude (Anthropic)**, or
- **OpenAI-compatible**: OpenAI, Gemini, DeepSeek, OpenRouter, Groq, or a local model through Ollama or LM Studio.

Pictures can come from **Gemini** or **OpenAI**. You can also add your own standing instructions ("Write in British English").

A local model through Ollama, for example:

```toml
[agent]
provider = "openai-compatible"
model = "qwen3"
api_key = "ollama"            # Ollama ignores it, but one is required
base_url = "http://localhost:11434/v1"
instructions = "Keep a friendly tone."
```

To try a prompt without opening the app:

```sh
cargo run -p inkwell-ai --example ask -- report.docx "make the title bold"
```

### How it works

The assistant never writes code or raw file contents. It can only call a fixed set of **typed tools**: insert paragraphs, rewrite a paragraph, format text, format the selection, replace the selection, change paragraph kinds, delete paragraphs, and insert a generated image. The editor already understands every one of these, so the assistant can't break the document.

It works on a copy of your document in the background. The page is read-only until it finishes, and then the copy is swapped in.

**Privacy:** each request sends the document's text to the AI service you've configured. Use a local model if that matters to you.

## Run it

Install Rust from [rustup.rs](https://rustup.rs), then:

```sh
git clone <this repo>
cd inkwell
cargo run --release                          # start with a blank document
cargo run --release -- path/to/report.docx   # open a file
```

On Linux you may need a few system packages first (Debian/Ubuntu):

```sh
sudo apt install libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

## Shortcuts

| Action | Shortcut |
| --- | --- |
| New / Open / Save / Save As | Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S |
| Bold / Italic / Underline | Ctrl+B / Ctrl+I / Ctrl+U |
| Undo / Redo | Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z |
| Find | Ctrl+F (Enter for next, Esc to close) |
| Select all | Ctrl+A |
| Start / end of document | Ctrl+Home / Ctrl+End |

On macOS, use Cmd instead of Ctrl.

## How it works

The code is split into four crates, so the interesting parts can be tested without a window:

| Crate | What it does |
| --- | --- |
| `inkwell-core` | The document model, editing commands, undo, find, and the page layout engine. No GUI code. |
| `inkwell-docx` | Reads and writes `.docx` files (which are zip files full of XML). |
| `inkwell-ai` | The assistant: provider adapters, the typed tools, and the request loop. No GUI code. |
| `inkwell-app` | The desktop app, built with [egui](https://github.com/emilk/egui). Paints the layout and turns input into editor commands. |

A few design choices worth knowing:

- **Every character stores its own style.** It uses a bit more memory than "runs" of text, but every edit becomes a simple list operation. Runs are only built when saving.
- **Undo saves a snapshot of the whole document** before each edit. Simple and reliable; fast enough for documents people write by hand.
- **Layout doesn't know about fonts.** It asks a `Measure` trait how wide each character is. The app answers using egui's fonts; the tests answer "10 points", which makes layout easy to test.
- **An image is one character.** Pictures live in an image table, and the text holds a single placeholder character for each one. Editing, undo, and line wrapping need no special cases.
- **Layout only reruns when the document changes.** The editor bumps a `revision` number on every edit, and the app caches layout by it.

## Known limitations

These are great first contributions:

- No tables, fonts, font sizes, colors, or alignment (they're dropped when opening a `.docx`)
- Images can't be resized, dragged, or pasted yet
- The assistant re-reads the whole document on each request, which costs more on long documents; replies don't stream yet
- Only one list level (no nested lists)
- Bold is drawn by painting text twice, since egui's built-in font has no bold face
- No spell check, page setup, headers/footers, or printing
- No right-to-left text or input method (IME) support

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
