use std::{env, fs, io, path::PathBuf};

use tui::style::Color;

/// Settings from the configuration file, `$XDG_CONFIG_HOME/parui/config` or by default
/// `~/.config/parui/config`. Each line is `key = value`, and lines starting with `#` are comments.
pub struct Settings {
    pub program: Option<String>,
    pub reopen: Reopen,
    pub notes: bool,
    pub notes_height: NotesHeight,
    pub colors: Colors,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            program: None,
            reopen: Reopen::Ask,
            notes: true,
            notes_height: NotesHeight::Percent(33),
            colors: Colors::default(),
        }
    }
}

/// What to do once the package command run on install, removal or upgrade is done.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reopen {
    /// Wait for Enter, then reopen parui, or quit on q.
    Ask,
    /// Reopen parui right away.
    Always,
    /// Leave the terminal to the package command, like upstream parui.
    Never,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotesHeight {
    Rows(u16),
    /// Percentage of the height of the right side.
    Percent(u16),
}

impl NotesHeight {
    /// Rows of the notes pane out of the `available` rows of the right side, keeping room for
    /// the borders of both panes.
    pub fn rows(self, available: u16) -> u16 {
        let rows = match self {
            NotesHeight::Rows(rows) => rows,
            NotesHeight::Percent(percent) => {
                u16::try_from(u32::from(available) * u32::from(percent) / 100).unwrap_or(u16::MAX)
            }
        };
        rows.clamp(3, available.saturating_sub(2).max(3))
    }
}

/// Colors of the package list. Explicit RGB by default, because terminal themes remap the ANSI
/// palette freely (Monokai Pro, for one, renders ANSI blue as orange), which would make the
/// package groups indistinguishable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Colors {
    pub upgradable: Color,
    pub installed: Color,
    pub not_installed: Color,
    pub note_label: Color,
    /// Text of the row under the cursor, whose background is the color of its group.
    pub cursor_text: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            upgradable: Color::Rgb(0xff, 0x9e, 0x40),
            installed: Color::Rgb(0x8c, 0xd8, 0x5a),
            not_installed: Color::Rgb(0x5c, 0xa8, 0xff),
            note_label: Color::Rgb(0x9a, 0x9a, 0x9a),
            cursor_text: Color::Rgb(0x1e, 0x1e, 0x1e),
        }
    }
}

pub fn path() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|dir| dir.join("parui/config"))
}

/// Loads the settings, which are all defaults if there is no configuration file.
pub fn load() -> Result<Settings, String> {
    let Some(path) = path() else {
        return Ok(Settings::default());
    };
    match fs::read_to_string(&path) {
        Ok(content) => parse(&content).map_err(|err| format!("{}:{err}", path.display())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

/// Parses a configuration file. Errors start with the line number they are about.
fn parse(content: &str) -> Result<Settings, String> {
    let mut settings = Settings::default();

    for (idx, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let err = |msg: String| format!("{}: {msg}", idx + 1);
        let Some((key, value)) = line.split_once('=') else {
            return Err(err(format!("expected `key = value`, found `{line}`")));
        };
        let (key, value) = (key.trim(), value.trim());
        let invalid = |expected: &str| err(format!("invalid {key} `{value}`, expected {expected}"));

        match key {
            "program" if !value.is_empty() => settings.program = Some(value.to_owned()),
            "program" => return Err(invalid("a command such as paru")),
            "reopen" => {
                settings.reopen = match value {
                    "ask" => Reopen::Ask,
                    "always" => Reopen::Always,
                    "never" => Reopen::Never,
                    _ => return Err(invalid("ask, always or never")),
                }
            }
            "notes" => {
                settings.notes = match value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(invalid("true or false")),
                }
            }
            "notes-height" => {
                let height = match value.strip_suffix('%') {
                    Some(percent) => percent
                        .parse()
                        .ok()
                        .filter(|percent| (1..=100).contains(percent))
                        .map(NotesHeight::Percent),
                    None => value.parse().ok().map(NotesHeight::Rows),
                };
                settings.notes_height =
                    height.ok_or_else(|| invalid("a number of rows or a percentage like 33%"))?;
            }
            _ => {
                let color = match key {
                    "color-upgradable" => &mut settings.colors.upgradable,
                    "color-installed" => &mut settings.colors.installed,
                    "color-not-installed" => &mut settings.colors.not_installed,
                    "color-note-label" => &mut settings.colors.note_label,
                    "color-cursor-text" => &mut settings.colors.cursor_text,
                    _ => return Err(err(format!("unknown key `{key}`"))),
                };
                *color = parse_color(value).ok_or_else(|| {
                    invalid("#rrggbb, a terminal color name such as green, or 0 to 255")
                })?;
            }
        }
    }

    Ok(settings)
}

/// Parses `#rrggbb`, a name of one of the 16 colors of the terminal theme, a 256-color palette
/// index, or `default` for the terminal's own text color.
fn parse_color(value: &str) -> Option<Color> {
    const NAMES: [&str; 8] = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];

    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() != 6 {
            return None;
        }
        let rgb = u32::from_str_radix(hex, 16).ok()?;
        let [_, r, g, b] = rgb.to_be_bytes();
        return Some(Color::Rgb(r, g, b));
    }
    if value == "default" {
        return Some(Color::Reset);
    }
    if let Ok(index) = value.parse() {
        return Some(Color::Indexed(index));
    }

    let (name, offset) = match value.strip_prefix("bright-") {
        Some(name) => (name, 8),
        None => (value, 0),
    };
    let index = NAMES.iter().position(|&n| n == name)?;
    Some(Color::Indexed(index as u8 + offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_settings() {
        let settings = parse(
            "# comment\n\
             \n\
             program = yay\n\
             reopen=never\n\
             notes = false\n\
             notes-height = 8\n\
             color-installed = green\n\
             color-upgradable = #FFaa00\n\
             color-not-installed = bright-blue\n\
             color-note-label = 245\n\
             color-cursor-text = default\n",
        )
        .unwrap();

        assert_eq!(settings.program.as_deref(), Some("yay"));
        assert_eq!(settings.reopen, Reopen::Never);
        assert!(!settings.notes);
        assert_eq!(settings.notes_height, NotesHeight::Rows(8));
        assert_eq!(
            settings.colors,
            Colors {
                upgradable: Color::Rgb(0xff, 0xaa, 0x00),
                installed: Color::Indexed(2),
                not_installed: Color::Indexed(12),
                note_label: Color::Indexed(245),
                cursor_text: Color::Reset,
            }
        );
        assert_eq!(
            parse("notes-height = 50%").unwrap().notes_height,
            NotesHeight::Percent(50)
        );
    }

    #[test]
    fn reports_errors_with_line_numbers() {
        assert_eq!(parse("\nfoo = bar").err().unwrap(), "2: unknown key `foo`");
        assert!(
            parse("reopen = maybe")
                .err()
                .unwrap()
                .starts_with("1: invalid reopen")
        );
        assert!(parse("color-installed = #12345").is_err());
        assert!(parse("color-installed = purple").is_err());
        assert!(parse("notes-height = 0%").is_err());
        assert!(parse("program").is_err());
    }

    #[test]
    fn sizes_notes_pane() {
        assert_eq!(NotesHeight::Percent(33).rows(30), 9);
        assert_eq!(NotesHeight::Rows(1).rows(30), 3);
        assert_eq!(NotesHeight::Rows(100).rows(30), 28);
        assert_eq!(NotesHeight::Percent(100).rows(5), 3);
    }
}
