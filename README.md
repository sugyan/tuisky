# tuisky: TUI client for Bluesky

[![](https://img.shields.io/crates/v/tuisky)](https://crates.io/crates/tuisky)
[![](https://img.shields.io/crates/l/atrium-api)](https://github.com/sugyan/tuisky/blob/main/LICENSE)

![out](https://github.com/user-attachments/assets/814291e9-8ed7-4bdf-ab4f-f62799f0c5c6)

## Features

- [x] Multiple columns, multiple session management
- [x] Select from pinned feeds
- [x] Auto refresh rows
- [x] Auto save & restore app data
- [x] Post texts
- [ ] Notifications, Chat, ...
- [x] Configure with files
- [ ] ... and more

## Installation

```
cargo install tuisky
```

### AUR

You can install `tuisky` from the [AUR](https://aur.archlinux.org/packages/tuisky) with using an [AUR helper](https://wiki.archlinux.org/title/AUR_helpers).

```
paru -S tuisky
```

## Usage

```
Usage: tuisky [OPTIONS]

Options:
  -c, --config <CONFIG>            Path to the configuration file
  -n, --num-columns <NUM_COLUMNS>  Maximum number of columns to display. The number of columns will be determined by the terminal width
  -h, --help                       Print help
  -V, --version                    Print version
```

### Default key bindings

Global:

- `Ctrl-q`: Quit
- `Ctrl-o`: Focus next column

Column:

- `Down`: Next item
- `Up`: Prev item
- `Enter`: Select item
- `Backspace`: Back to previous view
- `Ctrl-r`: Refresh current view
- `Ctrl-x`: Open/Close menu


### Configuration with toml file

Various settings can be read from a file.

```
tuisky --config path/to/config.toml
```

```toml
[keybindings.global]
Ctrl-c = "Quit"

[keybindings.column]
Ctrl-n = "NextItem"
Ctrl-p = "PrevItem"

[watcher.intervals]
feed_view_posts = 20
```

The config schema can be referenced by [JSON Schema](./config/tuisky.config.schema.json).
