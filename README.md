# Inkwell

A small, fast word processor written in Rust. It opens and saves real Word (`.docx`) files.

![Inkwell editing a document, with find results highlighted](docs/screenshot.png)

## Features

- Paginated "print layout" view: US Letter pages, one-inch margins, word wrap
- Bold, italic, underline
- Headings 1–3, bulleted lists, numbered lists
- Open and save `.docx` files that Word and LibreOffice read correctly
- Undo / redo (typing undoes in bursts, like Word)
- Find with highlighted matches
- Copy, cut, paste, double-click to select a word, zoom

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

The code is split into three crates, so the interesting parts can be tested without a window:

| Crate | What it does |
| --- | --- |
| `inkwell-core` | The document model, editing commands, undo, find, and the page layout engine. No GUI code. |
| `inkwell-docx` | Reads and writes `.docx` files (which are zip files full of XML). |
| `inkwell-app` | The desktop app, built with [egui](https://github.com/emilk/egui). Paints the layout and turns input into editor commands. |

A few design choices worth knowing:

- **Every character stores its own style.** It uses a bit more memory than "runs" of text, but every edit becomes a simple list operation. Runs are only built when saving.
- **Undo saves a snapshot of the whole document** before each edit. Simple and reliable; fast enough for documents people write by hand.
- **Layout doesn't know about fonts.** It asks a `Measure` trait how wide each character is. The app answers using egui's fonts; the tests answer "10 points", which makes layout easy to test.
- **Layout only reruns when the document changes.** The editor bumps a `revision` number on every edit, and the app caches layout by it.

## Known limitations

These are great first contributions:

- No tables, images, fonts, font sizes, colors, or alignment (they're dropped when opening a `.docx`)
- Only one list level (no nested lists)
- Bold is drawn by painting text twice, since egui's built-in font has no bold face
- No spell check, page setup, headers/footers, or printing
- No right-to-left text or input method (IME) support

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
