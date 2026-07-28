# Linux Guide

A TUI (Terminal User Interface) tool made in Rust to explore, learn, and recall Linux commands.

Online cheat.sh explanations · Offline whatis fallback · Real-time autocomplete ·
TUI mode · One-shot mode · Man page integration

---

## Features

- **Dual modes** — launch the full TUI or do a quick one-shot lookup from the terminal
- **Online explanations** — fetches rich, example-packed pages from cheat.sh (covers all UNIX/Linux commands, `git`, `docker`, programming languages, etc.)
- **Offline cache** — saves cheat.sh responses to `~/.cache/linux-guide/` so repeated lookups work without internet
- **Explain mode** — type a full command with flags (e.g. `git commit -m`) to get a breakdown of each flag from cheat.sh
- **Real-time autocomplete** — suggestions appear as you type (powered by `compgen -c`)
- **Ghost autocomplete** — inline gray text previews the top suggestion
- **Man page shortcut** — press `M` to open the full man page for any result
- **Syntax highlighting** — comments, commands, and errors are color-coded
- **Beautiful purple theme** — clean full-screen layout with no borders
- **Cross-platform** — works on Linux, macOS, and Windows (limited)

---

## Installation

### Prerequisites

You need **curl** installed (for fetching explanations from cheat.sh).

```bash
# Debian / Ubuntu
sudo apt install curl

# Fedora / RHEL
sudo dnf install curl

# macOS
brew install curl

# Windows (in PowerShell as admin)
winget install curl
```

### One-liner (Linux / macOS / Windows)

```bash
curl -fsSL https://raw.githubusercontent.com/RehanIlyas-dev/Linux-Guide/main/install.sh | bash
```
#### OR

### Cargo

```bash
cargo install linux-guide
```
#### OR

### Manual

Download the latest binary for your platform from the [Releases page](https://github.com/RehanIlyas-dev/Linux-Guide/releases), then:

```bash
chmod +x linux-guide-*
sudo mv linux-guide-* /usr/local/bin/linux-guide
```

---

## Usage

### TUI mode

Launch the interactive terminal UI:

```bash
linux-guide
```

Type a command name, press **Enter**, and get a detailed explanation fetched from cheat.sh. Suggestions appear as you type.



### One-shot mode

Get a quick explanation without entering the TUI:

```bash
linux-guide curl
```

Output is printed directly to the terminal and the program exits.

---

## Key bindings

| Key | Action |
|---|---|
| `Enter` | Look up the typed command |
| `Tab` | Accept the highlighted suggestion |
| `↑` / `↓` | Navigate suggestions / scroll output |
| `PgUp` / `PgDn` | Scroll output by 10 lines |
| `Home` | Scroll to top of output |
| `M` | Open the man page for the last result |
| `Esc` / `q` | Quit |

---

## How it works

1. **Autocomplete** — on startup, `compgen -c` (Linux) collects all available commands. As you type, the list is filtered and the top 10 matches are shown as a dropdown with an inline ghost preview.
2. **Search** — when you press Enter, the app runs `curl https://cheat.sh/<cmd>?T` to fetch a rich explanation with examples. The request runs in a background thread so the UI stays responsive. Results are cached to disk for offline use.
3. **Explain mode** — type a command with flags (e.g. `git log --oneline`) and each flag gets its own explanation from cheat.sh (`cheat.sh/git-log/--oneline`).
4. **Fallback** — if the network is unavailable and the cache is empty, the app falls back to `whatis <cmd>` for a concise one-line description.
5. **Man page** — press `M` to suspend the TUI, open the full system man page, and return when you close it.

### cheat.sh coverage

cheat.sh supports virtually all UNIX/Linux commands (`tar`, `grep`, `find`, `systemctl`, etc.), version control systems (`git`, `hg`, `svn`), container tools (`docker`, `podman`), programming languages (`python`, `go`, `rust`, `javascript`), and thousands more. Subcommands use hyphens (`git-commit`, `docker-run`) and work automatically when typed with spaces (e.g. `git commit`). You can also search by keyword with `~snapshot` syntax.

---

## Platform support

| Platform | Status |
|---|---|
| Linux | Full support (cheat.sh + whatis + compgen + man) |
| macOS | Supported (cheat.sh + whatis + man) |
| Windows | Basic (help command only) |

---

## Building from source

```bash
git clone https://github.com/RehanIlyas-dev/Linux-Guide.git
cd linux-guide
cargo build --release
./target/release/linux-guide
```

---

## Contributing

Contributions are welcome! Here's how:

1. **Fork** the repo on [GitHub](https://github.com/RehanIlyas-dev/Linux-Guide)
2. **Create a feature branch** (`git checkout -b feature/my-change`)
3. **Make your changes** and ensure they pass:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```
4. **Commit** (`git commit -am 'Add my feature'`)
5. **Push** (`git push origin feature/my-change`)
6. **Open a Pull Request** on GitHub

For bugs or feature requests, open an issue first.

---

## Author

Made with ❤️ by **Rehan**