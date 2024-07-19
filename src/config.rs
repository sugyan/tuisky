use crate::backend::config::Config as WatcherConfig;
use crate::components::views::types::Action as ViewAction;
use crate::types::Action as AppAction;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Config {
    pub num_columns: Option<usize>,
    #[serde(default)]
    pub dev: bool,
    #[serde(default)]
    pub keybindings: Keybindings,
    #[serde(default)]
    pub watcher: WatcherConfig,
}

impl Config {
    pub fn set_default_keybindings(&mut self) {
        // global: Ctrl-q to Quit
        self.keybindings
            .global
            .entry(Key(KeyCode::Char('q'), KeyModifiers::CONTROL))
            .or_insert(GlobalAction::Quit);
        // global: Ctrl-o to NextFocus
        self.keybindings
            .global
            .entry(Key(KeyCode::Char('o'), KeyModifiers::CONTROL))
            .or_insert(GlobalAction::NextFocus);
        // column: Down to NextItem
        self.keybindings
            .column
            .entry(Key(KeyCode::Down, KeyModifiers::NONE))
            .or_insert(ColumnAction::NextItem);
        // column: Up to PrevItem
        self.keybindings
            .column
            .entry(Key(KeyCode::Up, KeyModifiers::NONE))
            .or_insert(ColumnAction::PrevItem);
        // column: Tab to NextInput
        self.keybindings
            .column
            .entry(Key(KeyCode::Tab, KeyModifiers::NONE))
            .or_insert(ColumnAction::NextInput);
        // column: BackTab to PrevInput
        self.keybindings
            .column
            .entry(Key(KeyCode::BackTab, KeyModifiers::SHIFT))
            .or_insert(ColumnAction::NextInput);
        // column: Enter to Enter
        self.keybindings
            .column
            .entry(Key(KeyCode::Enter, KeyModifiers::NONE))
            .or_insert(ColumnAction::Enter);
        // column: Esc to Escape
        self.keybindings
            .column
            .entry(Key(KeyCode::Esc, KeyModifiers::NONE))
            .or_insert(ColumnAction::Escape);
        // column: Backspace to Back
        self.keybindings
            .column
            .entry(Key(KeyCode::Backspace, KeyModifiers::NONE))
            .or_insert(ColumnAction::Back);
        // column: Ctrl-r to Refresh
        self.keybindings
            .column
            .entry(Key(KeyCode::Char('r'), KeyModifiers::CONTROL))
            .or_insert(ColumnAction::Refresh);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Keybindings {
    pub global: HashMap<Key, GlobalAction>,
    pub column: HashMap<Key, ColumnAction>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Key(KeyCode, KeyModifiers);

impl From<KeyEvent> for Key {
    fn from(event: KeyEvent) -> Self {
        Self(event.code, event.modifiers)
    }
}

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let key_code = match self.0 {
            KeyCode::Char(c) => c.to_string(),
            _ => format!("{:?}", self.0),
        };
        if self.1 == KeyModifiers::NONE {
            key_code.serialize(serializer)
        } else {
            let modifier = match self.1 {
                KeyModifiers::CONTROL => "Ctrl",
                KeyModifiers::SHIFT => "Shift",
                _ => return Err(serde::ser::Error::custom("unsupported key modifier")),
            };
            format!("{modifier}-{key_code}").serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if let Some((modifier, code)) = s.split_once('-') {
            let mut chars = code.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                Ok(Self(
                    KeyCode::Char(c),
                    match modifier {
                        "Ctrl" => KeyModifiers::CONTROL,
                        "Shift" => KeyModifiers::SHIFT,
                        _ => return Err(serde::de::Error::custom("invalid key modifier")),
                    },
                ))
            } else {
                Err(serde::de::Error::custom("invalid key"))
            }
        } else {
            let key_code = match s.as_str() {
                "Backspace" => KeyCode::Backspace,
                "Enter" => KeyCode::Enter,
                "Left" => KeyCode::Left,
                "Right" => KeyCode::Right,
                "Up" => KeyCode::Up,
                "Down" => KeyCode::Down,
                "Home" => KeyCode::Home,
                "End" => KeyCode::End,
                "PageUp" => KeyCode::PageUp,
                "PageDown" => KeyCode::PageDown,
                "Tab" => KeyCode::Tab,
                "BackTab" => KeyCode::BackTab,
                "Delete" => KeyCode::Delete,
                "Insert" => KeyCode::Insert,
                "Esc" => KeyCode::Esc,
                _ if s.len() == 1 => KeyCode::Char(s.chars().next().unwrap()),
                _ => return Err(serde::de::Error::custom("unsupported key code")),
            };
            Ok(Self(key_code, KeyModifiers::NONE))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GlobalAction {
    NextFocus,
    PrevFocus,
    NewPost,
    Help,
    Quit,
}

impl From<&GlobalAction> for AppAction {
    fn from(action: &GlobalAction) -> Self {
        match action {
            GlobalAction::NextFocus => Self::NextFocus,
            GlobalAction::PrevFocus => Self::PrevFocus,
            GlobalAction::NewPost => Self::NewPost,
            GlobalAction::Help => Self::Help,
            GlobalAction::Quit => Self::Quit,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ColumnAction {
    NextItem,
    PrevItem,
    NextInput,
    PrevInput,
    Enter,
    Escape,
    Back,
    Refresh,
}

impl From<&ColumnAction> for ViewAction {
    fn from(action: &ColumnAction) -> Self {
        match action {
            ColumnAction::NextItem => Self::NextItem,
            ColumnAction::PrevItem => Self::PrevItem,
            ColumnAction::NextInput => Self::NextInput,
            ColumnAction::PrevInput => Self::PrevInput,
            ColumnAction::Enter => Self::Enter,
            ColumnAction::Escape => Self::Escape,
            ColumnAction::Back => Self::Back,
            ColumnAction::Refresh => Self::Refresh,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::config::Intervals;

    #[test]
    fn deserialize_empty() {
        let config = toml::from_str::<Config>("").expect("failed to deserialize config");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn deserialize() {
        let input = r#"
dev = true

[keybindings.global]
Ctrl-c = "Quit"
"?" = "Help"

[keybindings.column]
Ctrl-n = "NextItem"
Ctrl-p = "PrevItem"
Left = "Back"

[watcher.intervals]
feed = 20
"#;
        let config = toml::from_str::<Config>(input).expect("failed to deserialize config");
        assert_eq!(
            config,
            Config {
                dev: true,
                num_columns: None,
                keybindings: Keybindings {
                    global: HashMap::from_iter([
                        (
                            Key(KeyCode::Char('c'), KeyModifiers::CONTROL),
                            GlobalAction::Quit
                        ),
                        (
                            Key(KeyCode::Char('?'), KeyModifiers::NONE),
                            GlobalAction::Help
                        )
                    ]),
                    column: HashMap::from_iter([
                        (
                            Key(KeyCode::Char('n'), KeyModifiers::CONTROL),
                            ColumnAction::NextItem
                        ),
                        (
                            Key(KeyCode::Char('p'), KeyModifiers::CONTROL),
                            ColumnAction::PrevItem
                        ),
                        (Key(KeyCode::Left, KeyModifiers::NONE), ColumnAction::Back)
                    ]),
                },
                watcher: WatcherConfig {
                    intervals: Intervals {
                        preferences: 600,
                        feed: 20,
                        post_thread: 60,
                    }
                }
            }
        )
    }

    #[test]
    fn serialize() {
        let config = Config {
            num_columns: None,
            dev: true,
            keybindings: Keybindings {
                global: HashMap::from_iter([
                    (Key(KeyCode::Esc, KeyModifiers::NONE), GlobalAction::Quit),
                    (
                        Key(KeyCode::Char('c'), KeyModifiers::CONTROL),
                        GlobalAction::Quit,
                    ),
                    (
                        Key(KeyCode::Char('?'), KeyModifiers::NONE),
                        GlobalAction::Help,
                    ),
                ]),
                column: HashMap::new(),
            },
            watcher: WatcherConfig {
                intervals: Intervals {
                    feed: 10,
                    preferences: 10,
                    post_thread: 180,
                },
            },
        };
        let s = toml::to_string(&config).expect("failed to serialize config");
        let deserialized = toml::from_str::<Config>(&s).expect("failed to deserialize config");
        assert_eq!(deserialized, config);
    }
}
