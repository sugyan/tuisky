# tuisky: TUI application for Bluesky

[![](https://img.shields.io/crates/v/tuisky)](https://crates.io/crates/tuisky)
[![](https://img.shields.io/crates/l/atrium-api)](https://github.com/sugyan/tuisky/blob/main/LICENSE)


![demo.gif](https://github.com/sugyan/tuisky/assets/80381/e4820416-5e36-46f6-a154-d4f5a1f6ba64)

## Features

- [x] Multiple columns, multiple session management
- [x] Select from saved feeds
- [x] Auto refresh rows
- [x] Auto save & restore app data
- [ ] Post texts
- [ ] Notifications, Chat, ...
- [x] Configure with files
- [ ] ... and more

## Installation

```
cargo install tuisky
```

## Usage

```
Usage: tuisky [OPTIONS]

Options:
  -c, --config <CONFIG>            Path to the configuration file
  -n, --num-columns <NUM_COLUMNS>  Maximum number of columns to display. The number of columns will be determined by the terminal width
  -d, --dev                        Development mode
  -h, --help                       Print help
  -V, --version                    Print version
```

### Default key bindings

Global:

- `Ctrl - q`: Quit
- `Ctrl - o`: Focus next column

Column:

- `Down`: Next item
- `Up`: Prev item
- `Enter`: Select item
- `Backspace`: Back to previous view


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
