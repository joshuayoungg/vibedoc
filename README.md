# VibeDoc

A small, fast word processor written in Rust, with an AI assistant that edits your document for you, and shows you exactly what it changed. It opens and saves real Word (`.docx`) files.

![VibeDoc with the library on the left, document tabs and the formatting ribbon in the middle, and the assistant's review card on the right](docs/screenshot.png)

## Features

- Paginated "print layout" view: US Letter pages, one-inch margins, word wrap
- **Tabs**: several documents open at once; **+** opens a new one beside the others, and clicking the title renames it
- A formatting ribbon like Word's Home tab:
  - Fonts (Inter, Poppins, Lora, Merriweather, Playfair Display, JetBrains Mono) and sizes
  - Bold, italic, underline, strikethrough, superscript, subscript
  - Text color, highlight, clear formatting
  - Headings 1–3, quotes, code blocks, bulleted and numbered lists, checklists
  - Left, center, right, and justified alignment; line spacing
  - Pictures from a file
- Light and dark themes, and a choice of page color (File menu)
- **Saves automatically** a moment after you stop typing
- Open and save `.docx` files that Word and LibreOffice read correctly, with all of the formatting above
- **Export as PDF**, laid out exactly as on screen, with the fonts embedded
- Undo / redo (typing undoes in bursts, like Word)
- Find with highlighted matches
- Copy, cut, paste, double-click to select a word, zoom
- Images, including in saved `.docx` files
- **A document library**: folders and subfolders in a sidebar, with search, drag-and-drop, and renaming
- **An AI assistant** you can ask in plain English (side panel, or Ctrl+K anywhere), with a review card for every change

## The assistant

Ask for what you want, and the assistant makes the edit:

- "Put these notes into a bulleted list"
- "Make the selected text bold"
- "Draw a watercolor fox and put it here"
- "Add a title and a short intro, then make the second paragraph a heading"

Each request gets a **card** in the panel. The card shows:

- What you asked, and the assistant's reply
- How many paragraphs it added or rewritten (+) and removed (−)
- **See details**: the change as red (removed) and green (added) paragraphs, with formatting, while the changed paragraphs are highlighted on the page with a bar in the margin
- **Undo / Redo** for that request alone, even after later edits elsewhere in the document. If you've edited the same paragraphs since, the card says "Edited since" and leaves them alone.

Everything it does in one request is also **one undo step**, so Ctrl+Z takes it all back.

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
cargo run -p vibedoc-ai --example ask -- report.docx "make the title bold"
```

### How it works

The assistant never writes code or raw file contents. It can only call a fixed set of **typed tools**: insert paragraphs, rewrite a paragraph, format text, format the selection, replace the selection, change paragraph kinds, delete paragraphs, and insert a generated image. The editor already understands every one of these, so the assistant can't break the document.

It works on a copy of your document in the background. The page is read-only until it finishes, and then the copy is swapped in.

**Privacy:** each request sends the document's text to the AI service you've configured. Use a local model if that matters to you.

## Run it

Install Rust from [rustup.rs](https://rustup.rs), then:

```sh
git clone <this repo>
cd vibedoc
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
| New tab / Open / Save / Save As | Ctrl+N (or Ctrl+T) / Ctrl+O / Ctrl+S / Ctrl+Shift+S |
| Export as PDF | Ctrl+P |
| Close tab / Next tab / Previous tab | Ctrl+W / Ctrl+Tab / Ctrl+Shift+Tab |
| Bold / Italic / Underline / Strikethrough | Ctrl+B / Ctrl+I / Ctrl+U / Ctrl+Shift+X |
| Align left / center / right / justify | Ctrl+L / Ctrl+E / Ctrl+R / Ctrl+J |
| Bigger / smaller text | Ctrl+] / Ctrl+[ |
| Undo / Redo | Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z |
| Find | Ctrl+F (Enter for next, Esc to close) |
| Ask the assistant | Ctrl+K |
| Show / hide the library | Ctrl+\\ |
| Select all | Ctrl+A |
| Start / end of document | Ctrl+Home / Ctrl+End |

On macOS, use Cmd instead of Ctrl (Ctrl+Tab stays Ctrl).

## The File menu

**File** sits at the top left, next to Undo and Redo.

- **New document**, **Open…**, **Save**, **Save as…**, **Export as PDF…**
- **Make a copy**: saves "Name copy.docx" next to the original and opens it in a new tab
- **Rename** (or just click the title), **Show in Finder**, **Close tab**
- **Change library folder…** and **Show library in Finder**
- **Theme**: Light, Dark, or follow the system
- **Page color**: White, Ivory, Sepia, Mint, Slate, or Night
- **Assistant settings…**

## Saving

Documents save themselves about a second after you stop typing, so closing a tab or quitting never loses work. A new document saves into the library under its name as soon as it has some text; rename it by clicking the title. Use **Save as…** to keep a copy somewhere else. (VibeDoc only asks when a save fails, for example on a read-only disk.)

Theme, page color, and the library folder are kept in `preferences.toml` in VibeDoc's config folder.

## The library

The sidebar shows a folder on disk, `Documents/VibeDoc` by default (pick another with **File → Change library folder…**). Its subfolders and `.docx` files are the tree you see, folders first, so Finder and Explorer show the same organization.

- **+** next to search creates a document or a folder; right-click a folder for a new document or subfolder inside it
- Drag a document or folder onto another folder to move it, or onto the empty space below the tree to move it to the top level
- Right-click to rename, move, or delete
- Clicking a document opens it in a tab (or switches to it if it's already open)

## How it works

The code is split into five crates, so the interesting parts can be tested without a window:

| Crate | What it does |
| --- | --- |
| `vibedoc-core` | The document model, editing commands, undo, find, and the page layout engine. No GUI code. |
| `vibedoc-docx` | Reads and writes `.docx` files (which are zip files full of XML). |
| `vibedoc-pdf` | Exports PDFs from the same layout the app shows, using [krilla](https://github.com/LaurenzV/krilla). |
| `vibedoc-ai` | The assistant: provider adapters, the typed tools, and the request loop. No GUI code. |
| `vibedoc-app` | The desktop app, built with [egui](https://github.com/emilk/egui). Paints the layout and turns input into editor commands. |

A few design choices worth knowing:

- **Every character stores its own style.** It uses a bit more memory than "runs" of text, but every edit becomes a simple list operation. Runs are only built when saving.
- **Undo saves a snapshot of the whole document** before each edit. Simple and reliable; fast enough for documents people write by hand.
- **Layout doesn't know about fonts.** It asks a `Measure` trait how wide and tall each character is. The app answers using egui's fonts; the tests answer "10 points", which makes layout easy to test. Mixed fonts and sizes on one line share a baseline, like in Word.
- **The fonts ship with the app**, so a document looks the same on every computer. Fonts in a `.docx` from elsewhere become the closest bundled one (Times New Roman → Merriweather, Courier → JetBrains Mono, and so on).
- **Checklists are saved as a box character** (☐ or ☒) in front of the text, since Word has no checklist, so Word shows them and VibeDoc reads them back.
- **An image is one character.** Pictures live in an image table, and the text holds a single placeholder character for each one. Editing, undo, and line wrapping need no special cases.
- **Layout only reruns when the document changes.** The editor bumps a `revision` number on every edit, and the app caches layout by it.
- **Assistant changes are paragraph diffs.** Each card keeps the document from before and after its request. `vibedoc-core`'s `changes` module diffs them, finds those paragraphs in today's document, and swaps them back to undo. That's why one request can be undone after other edits.

## Known limitations

These are great first contributions:

- No tables, links, or comments (tables keep their text when opening a `.docx`)
- Fonts are limited to the six bundled ones
- Images can't be resized, dragged, or pasted yet
- The assistant re-reads the whole document on each request, which costs more on long documents; replies don't stream yet
- Only one list level (no nested lists)
- No spell check, page setup, headers/footers, or printing (export a PDF and print that)
- No right-to-left text or input method (IME) support

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).

The app bundles Inter, Poppins, Lora, Merriweather, Playfair Display, and JetBrains Mono, all under the SIL Open Font License. Each font's license is in its folder under `crates/vibedoc-app/assets/fonts/`.
