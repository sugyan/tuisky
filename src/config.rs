use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Config {
    pub keybindings: Option<Keybindings>,
    pub num_columns: Option<usize>,
    #[serde(default)]
    pub dev: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Keybindings {
    global: HashMap<Key, GlobalAction>,
    column: HashMap<Key, ColumnAction>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct Key(KeyCode, Option<KeyModifiers>);

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
                    format!("{modifier}+{c}").serialize(serializer)
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
        if let Some((modifier, code)) = s.split_once('+') {
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
enum GlobalAction {
    NextFocus,
    PrevFocus,
    Help,
    Quit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum ColumnAction {
    NextItem,
    PrevItem,
    NextInput,
    PrevInput,
    Enter,
    Back,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize() {
        let input = r#"
[keybindings.global]
"Ctrl+c" = "Quit"
"?" = "Help"

[keybindings.column]
"Ctrl+n" = "NextItem"
"Ctrl+p" = "PrevItem"

[watcher.intervals]
feed_view_posts = 20
"#;
        let config = toml::from_str::<Config>(input).expect("failed to deserialize config");
        assert_eq!(
            config,
            Config {
                keybindings: Some(Keybindings {
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
                }),
                num_columns: None,
                dev: false,
            }
        )
    }

    #[test]
    fn serialize() {
        let config = Config {
            keybindings: Some(Keybindings {
                global: HashMap::from_iter([
                    (
                        Key(KeyCode::Char('c'), Some(KeyModifiers::CONTROL)),
                        GlobalAction::Quit,
                    ),
                    (Key(KeyCode::Char('?'), None), GlobalAction::Help),
                ]),
                column: HashMap::from_iter([
                    (
                        Key(KeyCode::Char('n'), Some(KeyModifiers::CONTROL)),
                        ColumnAction::NextItem,
                    ),
                    (
                        Key(KeyCode::Char('p'), Some(KeyModifiers::CONTROL)),
                        ColumnAction::PrevItem,
                    ),
                ]),
            }),
            num_columns: None,
            dev: true,
        };
        let s = toml::to_string(&config).expect("failed to serialize config");
        let deserialized = toml::from_str::<Config>(&s).expect("failed to deserialize config");
        assert_eq!(deserialized, config);
    }
}
