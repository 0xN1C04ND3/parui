use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What a key does in select mode.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    Install,
    Remove,
    UpgradeAll,
    Ignore,
    Note,
    Select,
    ClearSelection,
    Search,
    Quit,
    Down,
    Up,
    PageDown,
    PageUp,
    Top,
    Bottom,
    InfoDown,
    InfoUp,
}

impl Action {
    pub const ALL: [Action; 17] = [
        Action::Install,
        Action::Remove,
        Action::UpgradeAll,
        Action::Ignore,
        Action::Note,
        Action::Select,
        Action::ClearSelection,
        Action::Search,
        Action::Quit,
        Action::Down,
        Action::Up,
        Action::PageDown,
        Action::PageUp,
        Action::Top,
        Action::Bottom,
        Action::InfoDown,
        Action::InfoUp,
    ];

    /// Name in the configuration file, after `key-`.
    pub fn name(self) -> &'static str {
        match self {
            Action::Install => "install",
            Action::Remove => "remove",
            Action::UpgradeAll => "upgrade-all",
            Action::Ignore => "ignore",
            Action::Note => "note",
            Action::Select => "select",
            Action::ClearSelection => "clear-selection",
            Action::Search => "search",
            Action::Quit => "quit",
            Action::Down => "down",
            Action::Up => "up",
            Action::PageDown => "page-down",
            Action::PageUp => "page-up",
            Action::Top => "top",
            Action::Bottom => "bottom",
            Action::InfoDown => "info-down",
            Action::InfoUp => "info-up",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.name() == name)
    }

    fn default_keys(self) -> &'static [&'static str] {
        match self {
            Action::Install => &["enter"],
            Action::Remove => &["R"],
            Action::UpgradeAll => &["U"],
            Action::Ignore => &["x"],
            Action::Note => &["n"],
            Action::Select => &["space"],
            Action::ClearSelection => &["c"],
            Action::Search => &["i", "/"],
            Action::Quit => &["q"],
            Action::Down => &["j", "down"],
            Action::Up => &["k", "up"],
            Action::PageDown => &["l", "right", "pagedown"],
            Action::PageUp => &["h", "left", "pageup"],
            Action::Top => &["g", "home"],
            Action::Bottom => &["G", "end"],
            Action::InfoDown => &["ctrl+j", "ctrl+down"],
            Action::InfoUp => &["ctrl+k", "ctrl+up"],
        }
    }
}

const NAMED_KEYS: [(&str, KeyCode); 14] = [
    ("enter", KeyCode::Enter),
    ("space", KeyCode::Char(' ')),
    ("tab", KeyCode::Tab),
    ("backspace", KeyCode::Backspace),
    ("delete", KeyCode::Delete),
    ("insert", KeyCode::Insert),
    ("up", KeyCode::Up),
    ("down", KeyCode::Down),
    ("left", KeyCode::Left),
    ("right", KeyCode::Right),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("pageup", KeyCode::PageUp),
    ("pagedown", KeyCode::PageDown),
];

/// A key with its modifiers. Shift is part of the character for character keys, so `R` is
/// Shift-r.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl Key {
    fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        let mut modifiers =
            modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
        let code = match code {
            KeyCode::Char(c) if modifiers.contains(KeyModifiers::SHIFT) => {
                modifiers.remove(KeyModifiers::SHIFT);
                KeyCode::Char(c.to_uppercase().next().unwrap_or(c))
            }
            code => code,
        };
        Self { code, modifiers }
    }

    pub fn from_event(event: &KeyEvent) -> Self {
        Self::new(event.code, event.modifiers)
    }

    /// Parses keys such as `x`, `R`, `enter`, `ctrl+j` or `alt+shift+left`. `,` and `+` are
    /// written `comma` and `plus`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let key = parts.pop().unwrap_or_default();

        let mut modifiers = KeyModifiers::NONE;
        for modifier in parts {
            modifiers |= match modifier.to_lowercase().as_str() {
                "ctrl" => KeyModifiers::CONTROL,
                "alt" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                _ => return Err(format!("unknown modifier `{modifier}` in `{s}`")),
            };
        }

        let lower = key.to_lowercase();
        let mut chars = key.chars();
        let code = match (chars.next(), chars.next()) {
            (Some(c), None) => KeyCode::Char(c),
            _ => match lower.as_str() {
                "comma" => KeyCode::Char(','),
                "plus" => KeyCode::Char('+'),
                "esc" | "escape" => {
                    return Err("Escape always switches modes and cannot be bound".to_owned());
                }
                name => {
                    if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse().ok()) {
                        KeyCode::F(n)
                    } else if let Some((_, code)) = NAMED_KEYS.iter().find(|(n, _)| *n == name) {
                        *code
                    } else {
                        return Err(format!("unknown key `{key}`"));
                    }
                }
            },
        };

        let key = Self::new(code, modifiers);
        if key == Self::new(KeyCode::Char('c'), KeyModifiers::CONTROL) {
            return Err("ctrl+c always quits and cannot be bound".to_owned());
        }
        Ok(key)
    }

    /// How the key is shown in the interface, such as `Enter`, `R` or `Ctrl-j`.
    pub fn label(&self) -> String {
        let mut label = String::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            label.push_str("Ctrl-");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            label.push_str("Alt-");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            label.push_str("Shift-");
        }
        match self.code {
            KeyCode::Char(' ') => label.push_str("Space"),
            KeyCode::Char(c) => label.push(c),
            KeyCode::F(n) => label.push_str(&format!("F{n}")),
            KeyCode::PageUp => label.push_str("PgUp"),
            KeyCode::PageDown => label.push_str("PgDn"),
            code => label.push_str(&format!("{code:?}")),
        }
        label
    }
}

/// Which action each key triggers in select mode.
pub struct Keymap {
    actions: HashMap<Key, Action>,
    keys: HashMap<Action, Vec<Key>>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new(HashMap::new()).expect("the default keys do not conflict")
    }
}

impl Keymap {
    /// The default keys, with the keys of the actions in `overrides` replaced.
    pub fn new(mut overrides: HashMap<Action, Vec<Key>>) -> Result<Self, String> {
        let mut keymap = Self {
            actions: HashMap::new(),
            keys: HashMap::new(),
        };

        for action in Action::ALL {
            let keys = overrides.remove(&action).unwrap_or_else(|| {
                action
                    .default_keys()
                    .iter()
                    .map(|key| Key::parse(key).expect("default keys are valid"))
                    .collect()
            });

            for &key in &keys {
                if let Some(other) = keymap.actions.insert(key, action) {
                    return Err(format!(
                        "{} is bound to both {} and {}, change key-{} or key-{}",
                        key.label(),
                        other.name(),
                        action.name(),
                        other.name(),
                        action.name(),
                    ));
                }
            }
            keymap.keys.insert(action, keys);
        }

        Ok(keymap)
    }

    pub fn action(&self, event: &KeyEvent) -> Option<Action> {
        self.actions.get(&Key::from_event(event)).copied()
    }

    /// Label of the first key of `action`, or `None` if it is unbound.
    pub fn label(&self, action: Action) -> Option<String> {
        self.keys[&action].first().map(Key::label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn parses_keys() {
        assert_eq!(Key::parse("x").unwrap().label(), "x");
        assert_eq!(Key::parse("R").unwrap(), Key::parse("shift+r").unwrap());
        assert_eq!(Key::parse("ctrl+j").unwrap().label(), "Ctrl-j");
        assert_eq!(Key::parse("Enter").unwrap().label(), "Enter");
        assert_eq!(
            Key::parse("alt+shift+left").unwrap().label(),
            "Alt-Shift-Left"
        );
        assert_eq!(Key::parse("comma").unwrap().label(), ",");
        assert_eq!(Key::parse("f5").unwrap().label(), "F5");
        assert!(Key::parse("esc").is_err());
        assert!(Key::parse("ctrl+c").is_err());
        assert!(Key::parse("hyper+x").is_err());
        assert!(Key::parse("nope").is_err());
    }

    #[test]
    fn maps_events_to_actions() {
        let keymap = Keymap::default();
        assert_eq!(
            keymap.action(&event(KeyCode::Char('R'), KeyModifiers::SHIFT)),
            Some(Action::Remove)
        );
        assert_eq!(
            keymap.action(&event(KeyCode::Char('j'), KeyModifiers::CONTROL)),
            Some(Action::InfoDown)
        );
        assert_eq!(
            keymap.action(&event(KeyCode::Char('j'), KeyModifiers::NONE)),
            Some(Action::Down)
        );
        assert_eq!(
            keymap.action(&event(KeyCode::Char('z'), KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn overrides_and_conflicts() {
        let keymap = Keymap::new(HashMap::from([
            (Action::Ignore, vec![Key::parse("I").unwrap()]),
            (Action::Note, vec![]),
        ]))
        .unwrap();
        assert_eq!(
            keymap.action(&event(KeyCode::Char('I'), KeyModifiers::SHIFT)),
            Some(Action::Ignore)
        );
        assert_eq!(
            keymap.action(&event(KeyCode::Char('x'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(keymap.label(Action::Note), None);

        let err = Keymap::new(HashMap::from([(
            Action::Ignore,
            vec![Key::parse("n").unwrap()],
        )]))
        .err()
        .unwrap();
        assert!(
            err.starts_with("n is bound to both ignore and note"),
            "{err}"
        );
    }
}
