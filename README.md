# txtbatch

A batch text find/replace/insert tool for all text files in a directory, with both CLI and GUI interfaces.

## Features

- **Find & Replace** — replace all occurrences of text across files
- **Insert After** — insert text after every occurrence of a pattern
- **GUI Preview** — real-time diff preview with colored before/after view before applying changes
- **Dry Run** — preview changes without writing to disk
- **Binary Detection** — automatically skips binary files
- **Directory Memory** — remembers the last used directory
- **Recursive** — processes all files in subdirectories

## Installation

### Build from source

```bash
git clone https://github.com/Schallophy/txtbatch.git
cd txtbatch
cargo build --release
```

Binaries are produced at:
- `target/release/cli.exe` — command-line interface
- `target/release/gui.exe` — graphical interface (Windows, no console window)

## Usage

### CLI

```bash
# Replace text in a directory
cli.exe --dir /path/to/project replace "old_text" "new_text"

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

### GUI

```bash
# Launch the graphical interface
gui.exe
```

The GUI provides:
- Directory selector with browse dialog
- Replace / Insert mode toggle
- Input fields for search and replacement text
- Preview button to see all changes as a colored diff
- Confirm / Cancel buttons to apply or discard changes

## License

[MIT](LICENSE)
