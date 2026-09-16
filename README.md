# txtbatch

A batch text find/replace/insert tool for all text files in a directory, with CLI and Tauri desktop interfaces.

## Features

- **Find & Replace** — replace all occurrences of text across files
- **Repeat** — repeat every occurrence of text a given number of times
- **Insert After** — insert text after every occurrence of a pattern
- **Tauri GUI** — native desktop shell with real-time diff preview
- **Dry Run** — preview changes without writing to disk
- **Binary Detection** — automatically skips binary files
- **Directory Memory** — remembers the last used directory
- **Recursive** — processes all files in subdirectories

## Installation

### Build from source

```bash
git clone https://github.com/Schallophy/txtbatch.git
cd txtbatch
```

**CLI** — pure Rust, built with Cargo:

```bash
cargo build --release
```

Produces `target/release/cli.exe`.

**Desktop app** — Tauri + Vue, requires Node.js and the [Tauri prerequisites](https://tauri.app/start/prerequisites/):

```bash
npm install
npm run tauri build
```

Produces `src-tauri/target/release/txtbatch-app.exe`. On Windows the desktop app also requires the WebView2 runtime, which is preinstalled on Windows 11 and most Windows 10 systems.

## Usage

### CLI

```bash
# Replace text in a directory
cli.exe --dir /path/to/project replace "old_text" "new_text"

# Repeat every match 3 times (e.g. `123` -> `123123123`)
cli.exe --dir /path/to/project repeat "123" 3

# Insert text after a pattern
cli.exe --dir /path/to/project insert "anchor_text" "text_to_insert"

# Preview only (no changes written)
cli.exe --dir /path/to/project --dry-run replace "foo" "bar"

# Show detailed diff
cli.exe --dir /path/to/project --diff replace "foo" "bar"

# Set default directory (can omit --dir afterwards)
cli.exe set-dir /path/to/project

# Show current default directory
cli.exe show-dir

# Clear default directory
cli.exe clear-dir
```

### Tauri GUI

```bash
# Install frontend dependencies
npm install

# Launch the desktop app in development mode
npm run tauri dev
```

The desktop app provides:
- Directory selector with browse dialog
- Replace / Repeat / Insert mode toggle
- Input fields for the search text and its replacement, repeat count, or inserted text
- Preview button to see all changes as a colored diff
- Apply / Clear buttons to apply or discard changes

See [Build from source](#build-from-source) for the production build.

## License

[MIT](LICENSE)
