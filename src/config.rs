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
        // global: Ctrl+q to Quit
        self.keybindings
            .global
            .entry(Key(KeyCode::Char('q'), Some(KeyModifiers::CONTROL)))
            .or_insert(GlobalAction::Quit);
        // global: Ctrl+o to NextFocus
        self.keybindings
            .global
            .entry(Key(KeyCode::Char('o'), Some(KeyModifiers::CONTROL)))
            .or_insert(GlobalAction::NextFocus);
        // column: Down to NextItem
        self.keybindings
            .column
            .entry(Key(KeyCode::Down, None))
            .or_insert(ColumnAction::NextItem);
        // column: Up to PrevItem
        self.keybindings
            .column
            .entry(Key(KeyCode::Up, None))
            .or_insert(ColumnAction::PrevItem);
        // column: Tab to NextInput
        self.keybindings
            .column
            .entry(Key(KeyCode::Tab, None))
            .or_insert(ColumnAction::NextInput);
        // column: BackTab to PrevInput
        self.keybindings
            .column
            .entry(Key(KeyCode::BackTab, Some(KeyModifiers::SHIFT)))
            .or_insert(ColumnAction::NextInput);
        // column: Enter to Enter
        self.keybindings
            .column
            .entry(Key(KeyCode::Enter, None))
            .or_insert(ColumnAction::Enter);
        // column: Backspace to Back
        self.keybindings
            .column
            .entry(Key(KeyCode::Backspace, None))
            .or_insert(ColumnAction::Back);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Keybindings {
    pub global: HashMap<Key, GlobalAction>,
    pub column: HashMap<Key, ColumnAction>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Key(KeyCode, Option<KeyModifiers>);

impl From<KeyEvent> for Key {
    fn from(event: KeyEvent) -> Self {
        Self(
            event.code,
            match event.modifiers {
                KeyModifiers::CONTROL | KeyModifiers::SHIFT => Some(event.modifiers),
                _ => None,
            },
        )
    }
}

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            KeyCode::Char(c) => match self.1 {
                Some(modifier) => {
                    let modifier = match modifier {
                        KeyModifiers::CONTROL => "Ctrl",
                        KeyModifiers::SHIFT => "Shift",
                        _ => return Err(serde::ser::Error::custom("invalid key modifier")),
                    };
                    format!("{modifier}-{c}").serialize(serializer)
                }
                None => c.to_string().serialize(serializer),
            },
            _ => Err(serde::ser::Error::custom("invalid key code")),
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
                        "Ctrl" => Some(KeyModifiers::CONTROL),
                        "Shift" => Some(KeyModifiers::SHIFT),
                        _ => return Err(serde::de::Error::custom("invalid key modifier")),
                    },
                ))
            } else {
                Err(serde::de::Error::custom("invalid key"))
            }
        } else {
            let mut chars = s.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                Ok(Self(KeyCode::Char(c), None))
            } else {
                Err(serde::de::Error::custom("invalid key"))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GlobalAction {
    NextFocus,
    PrevFocus,
    Help,
    Quit,
}

impl From<&GlobalAction> for AppAction {
    fn from(action: &GlobalAction) -> Self {
        match action {
            GlobalAction::NextFocus => Self::NextFocus,
            GlobalAction::PrevFocus => Self::PrevFocus,
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
    Back,
}

impl From<&ColumnAction> for ViewAction {
    fn from(action: &ColumnAction) -> Self {
        match action {
            ColumnAction::NextItem => Self::NextItem,
            ColumnAction::PrevItem => Self::PrevItem,
            ColumnAction::NextInput => Self::NextInput,
            ColumnAction::PrevInput => Self::PrevInput,
            ColumnAction::Enter => Self::Enter,
            ColumnAction::Back => Self::Back,
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

[watcher.intervals]
feed_view_posts = 20
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
                            Key(KeyCode::Char('c'), Some(KeyModifiers::CONTROL)),
                            GlobalAction::Quit
                        ),
                        (Key(KeyCode::Char('?'), None), GlobalAction::Help)
                    ]),
                    column: HashMap::from_iter([
                        (
                            Key(KeyCode::Char('n'), Some(KeyModifiers::CONTROL)),
                            ColumnAction::NextItem
                        ),
                        (
                            Key(KeyCode::Char('p'), Some(KeyModifiers::CONTROL)),
                            ColumnAction::PrevItem
                        )
                    ]),
                },
                watcher: WatcherConfig {
                    intervals: Intervals {
                        feed_view_posts: 20,
                        preferences: 60,
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
                    (
                        Key(KeyCode::Char('c'), Some(KeyModifiers::CONTROL)),
                        GlobalAction::Quit,
                    ),
                    (Key(KeyCode::Char('?'), None), GlobalAction::Help),
                ]),
                column: HashMap::new(),
            },
            watcher: WatcherConfig {
                intervals: Intervals {
                    feed_view_posts: 10,
                    preferences: 10,
                },
            },
        };
        let s = toml::to_string(&config).expect("failed to serialize config");
        let deserialized = toml::from_str::<Config>(&s).expect("failed to deserialize config");
        assert_eq!(deserialized, config);
    }
}
