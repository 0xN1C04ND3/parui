use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs::{File, FileType},
    io::{BufRead, BufReader, Seek},
    sync::Arc,
    time::Duration,
};

use compact_strings::FixedCompactStrings;
use flate2::bufread::GzDecoder;
use nohash_hasher::{IntMap, IntSet};
use parking_lot::RwLock;
use regex::Regex;
use tokio::{join, process::Command, time::sleep};
use tui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::{config::Colors, ignore::Ignored, matcher::Matcher, notes::Notes, shown::Shown};

pub async fn list(show_aur: bool) -> FixedCompactStrings {
    let mut cmd = Command::new("pacman");
    cmd.arg("-Slq");

    let pacman_out = cmd.output();
    let aur_out = tokio::task::spawn_blocking(move || {
        if show_aur {
            ureq::get("https://aur.archlinux.org/packages.gz")
                .call()
                .ok()
        } else {
            None
        }
    });

    let (pacman_out, aur_out) = join!(pacman_out, aur_out);

    let mut out = FixedCompactStrings::with_capacity(16 * 16384, 16384);

    let Ok(pacman_out) = pacman_out else {
        return out;
    };

    let Ok(aur_out) = aur_out else {
        return out;
    };

    out.extend(
        pacman_out
            .stdout
            .split(|&b| b == b'\n')
            .filter(|line| !line.is_empty())
            .filter_map(|line| std::str::from_utf8(line).ok()),
    );

    if let Some(aur_out) = aur_out {
        let reader = BufReader::new(aur_out.into_body().into_reader());
        let decoder = GzDecoder::new(reader);

        if let Ok(s) = std::io::read_to_string(decoder) {
            out.extend(
                s.as_bytes()
                    .split(|&b| b == b'\n')
                    .filter(|line| !line.is_empty())
                    .filter_map(|line| std::str::from_utf8(line).ok()),
            );
        }
    }

    out.shrink_to_fit();
    out.shrink_meta_to_fit();

    out
}

#[derive(Clone)]
pub struct Upgrade {
    pub old: String,
    pub new: String,
}

/// Packages with a newer version available, keyed by name, as reported by `<command> -Qu`.
///
/// Repository packages are compared against the local sync database, so this is only as fresh
/// as the last `pacman -Sy`.
pub async fn list_upgradable(command: &str) -> HashMap<String, Upgrade> {
    let mut cmd = Command::new(command);
    cmd.arg("-Qu");

    parse_upgrades(&cmd_output(cmd).await)
}

/// Parses `-Qu` output, where each line looks like `name old -> new`, optionally followed by
/// ` [ignored]`.
fn parse_upgrades(output: &str) -> HashMap<String, Upgrade> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let old = parts.next()?;
            if parts.next()? != "->" {
                return None;
            }
            let new = parts.next()?;
            Some((
                name.to_owned(),
                Upgrade {
                    old: old.to_owned(),
                    new: new.to_owned(),
                },
            ))
        })
        .collect()
}

pub struct Grouped {
    pub packages: FixedCompactStrings,
    pub installed: IntSet<usize>,
    pub upgradable: IntMap<usize, Upgrade>,
}

/// Reorders `packages` into upgradable, then installed, then not installed packages, and returns
/// the indices of each group in the new order. The installed groups are sorted by name, while
/// the not installed packages keep their original order.
///
/// `installed` contains the upgradable packages too.
pub fn group_packages(
    packages: &FixedCompactStrings,
    installed: &HashSet<String>,
    upgrades: &HashMap<String, Upgrade>,
) -> Grouped {
    #[derive(PartialEq, Eq, Clone, Copy)]
    enum Group {
        Upgradable,
        Installed,
        NotInstalled,
    }

    let group_of = |package: &str| {
        if !installed.contains(package) {
            Group::NotInstalled
        } else if upgrades.contains_key(package) {
            Group::Upgradable
        } else {
            Group::Installed
        }
    };
    let groups: Vec<Group> = packages.iter().map(group_of).collect();

    let mut out = Grouped {
        packages: FixedCompactStrings::with_capacity(packages.capacity(), packages.len()),
        installed: IntSet::default(),
        upgradable: IntMap::default(),
    };

    for group in [Group::Upgradable, Group::Installed, Group::NotInstalled] {
        let mut members: Vec<&str> = packages
            .iter()
            .zip(&groups)
            .filter(|(_, g)| **g == group)
            .map(|(package, _)| package)
            .collect();
        // The listing goes through each repository alphabetically, then the AUR. Sort the
        // installed groups into one alphabetical run, but keep the repository order of the rest
        // so that official packages stay ahead of the AUR.
        if group != Group::NotInstalled {
            members.sort();
        }

        for package in members {
            let idx = out.packages.len();
            match group {
                Group::Upgradable => {
                    out.installed.insert(idx);
                    out.upgradable.insert(idx, upgrades[package].clone());
                }
                Group::Installed => {
                    out.installed.insert(idx);
                }
                Group::NotInstalled => (),
            }
            out.packages.push(package);
        }
    }

    out
}

pub fn search(query: &str, packages: &FixedCompactStrings, shown: Arc<RwLock<Shown>>) {
    let query = query.trim();
    if query.is_empty() || query == ".*" || query == "." {
        *shown.write() = Shown::All
    } else {
        fn inner<'a>(
            matcher: &(impl Matcher<&'a str> + ?Sized),
            packages: &'a FixedCompactStrings,
            shown: Arc<RwLock<Shown>>,
        ) {
            let mut handle = shown.write();
            match *handle {
                Shown::Few(_) => {
                    handle.clear();
                    handle.extend(
                        packages
                            .iter()
                            .enumerate()
                            .filter(|(_, package)| matcher.matches(*package))
                            .map(|(i, _)| i),
                    )
                }
                _ => {
                    *handle = Shown::Few(
                        packages
                            .iter()
                            .enumerate()
                            .filter(|(_, package)| matcher.matches(*package))
                            .map(|(i, _)| i)
                            .collect(),
                    )
                }
            }
        }

        if let Ok(re) = Regex::new(query) {
            inner(&re, packages, shown);
        } else {
            inner(query, packages, shown);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn format_results<'line>(
    packages: &'line FixedCompactStrings,
    shown: Arc<RwLock<Shown>>,
    current: usize,
    selected: &IntSet<usize>,
    height: usize,
    pad_to: usize,
    skip: usize,
    installed: &IntSet<usize>,
    upgradable: &IntMap<usize, Upgrade>,
    ignored: &Ignored,
    notes: &Notes,
    colors: &Colors,
) -> Vec<Line<'line>> {
    use crate::{cows, style};

    const INDEX_STYLE: Style = style!(Color::Gray);
    const DEFAULT_STYLE: Style = style!();

    let package_style = |idx: usize, name: &str, is_current: bool| {
        let color = if upgradable.contains_key(&idx) {
            colors.upgradable
        } else if installed.contains(&idx) {
            colors.installed
        } else {
            colors.not_installed
        };

        let style = Style::new().add_modifier(Modifier::BOLD);
        let style = if is_current {
            style.fg(colors.cursor_text).bg(color)
        } else {
            style.fg(color)
        };

        if ignored.matches(name) {
            style.add_modifier(Modifier::CROSSED_OUT)
        } else {
            style
        }
    };

    const PADDINGS: [Cow<'static, str>; 16] = cows!(
        "",
        " ",
        "  ",
        "   ",
        "    ",
        "     ",
        "      ",
        "       ",
        "        ",
        "         ",
        "          ",
        "           ",
        "            ",
        "             ",
        "              ",
        "               "
    );

    let has_note = Span::styled(
        " Note",
        Style::new()
            .fg(colors.note_label)
            .add_modifier(Modifier::ITALIC),
    );

    const SELECTED: Span = Span {
        content: Cow::Borrowed("!"),
        style: style! { fg: Color::Yellow, mod: Modifier::BOLD, },
    };

    match shown.read().get_vec() {
        Some(shown) => shown
            .iter()
            .skip(skip)
            .take(height - 5)
            .copied()
            .enumerate()
            .map(|(i, package_idx)| {
                let real_index = shown[skip + i];
                let index = i + skip + 1;

                let index_span = Span::styled(index.to_string(), INDEX_STYLE);
                let padding_span = Span {
                    content: PADDINGS[pad_to - index.ilog10() as usize].clone(),
                    style: DEFAULT_STYLE,
                };
                let line_span = Span::styled(
                    &packages[package_idx],
                    package_style(real_index, &packages[package_idx], current == index - 1),
                );

                let mut spans = vec![index_span, padding_span, line_span];
                if notes.contains(&packages[package_idx]) {
                    spans.push(has_note.clone());
                }
                if selected.contains(&real_index) {
                    spans.push(SELECTED);
                }
                Line::from(spans)
            })
            .collect(),
        None => packages
            .iter()
            .enumerate()
            .skip(skip)
            .take(height - 5)
            .map(|(i, line)| {
                let index_span = Span::styled((i + 1).to_string(), INDEX_STYLE);
                let padding_span = Span {
                    content: PADDINGS[pad_to - (i + 1).ilog10() as usize].clone(),
                    style: DEFAULT_STYLE,
                };
                let line_span = Span::styled(line, package_style(i, line, current == i));

                let mut spans = vec![index_span, padding_span, line_span];
                if notes.contains(line) {
                    spans.push(has_note.clone());
                }
                if selected.contains(&i) {
                    spans.push(SELECTED);
                }
                Line::from(spans)
            })
            .collect(),
    }
}

pub async fn get_info<'line>(
    all_packages: &FixedCompactStrings,
    index: usize,
    installed_cache: &IntSet<usize>,
    upgradable: &IntMap<usize, Upgrade>,
    colors: Colors,
    command: &str,
) -> Vec<Line<'line>> {
    if index >= all_packages.len() {
        return Vec::new();
    }

    let mut cmd = Command::new(command);

    if installed_cache.contains(&index) {
        cmd.arg("-Qi");
    } else {
        // Debounce so that we don't spam requests
        sleep(Duration::from_millis(200)).await;

        cmd.arg("-Si");
    };

    cmd.arg(&all_packages[index]);

    let output = cmd_output(cmd).await;
    let lines = output.lines().map(ToOwned::to_owned).collect::<Vec<_>>();

    const KEY_STYLE: Style = Style {
        fg: None,
        bg: None,
        underline_color: None,
        add_modifier: Modifier::BOLD,
        sub_modifier: Modifier::empty(),
    };

    let mut info = Vec::with_capacity(lines.len() + 1);
    if let Some(upgrade) = upgradable.get(&index) {
        info.push(Line::from(vec![
            Span::styled("Upgrade available:", KEY_STYLE),
            Span::styled(
                format!(" {} -> {}", upgrade.old, upgrade.new),
                Style::new()
                    .fg(colors.upgradable)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        info.push(Line::default());
    }

    for mut line in lines {
        if line.starts_with(' ') {
            info.push(Line::from(line));
            continue;
        }

        if let Some(idx) = line.find(':') {
            let value = line.split_off(idx + 1);
            info.push(Line::from(vec![
                Span::styled(line, KEY_STYLE),
                Span::raw(value),
            ]));
        }
    }

    info
}

/// Names of all installed packages, read from the local pacman database.
pub fn installed_names() -> HashSet<String> {
    const DIR: &str = "/var/lib/pacman/local/";

    let Ok(dir) = std::fs::read_dir(DIR) else {
        return HashSet::default();
    };

    let mut set = HashSet::with_capacity(512);

    let mut path = Vec::with_capacity(256);
    let dir_os = OsStr::new(DIR);
    let dir_os_bytes = dir_os.as_encoded_bytes();
    let dir_len = dir_os_bytes.len();
    path.extend_from_slice(dir_os_bytes);

    let mut reader: Option<BufReader<File>> = None;
    for entry in dir.filter_map(Result::ok) {
        let Ok(true) = entry.file_type().as_ref().map(FileType::is_dir) else {
            continue;
        };

        path.extend_from_slice(entry.file_name().as_encoded_bytes());
        path.extend_from_slice(OsStr::new("/desc").as_encoded_bytes());

        // SAFETY: `path` only contains content that originated from `OsStr::as_encoded_bytes`
        let Ok(file) = File::open(unsafe { OsStr::from_encoded_bytes_unchecked(&path) }) else {
            continue;
        };

        path.truncate(dir_len);

        match reader {
            Some(ref mut reader) => {
                reader.rewind().unwrap();
                *reader.get_mut() = file;
            }
            None => {
                reader = Some(BufReader::new(file));
            }
        }

        if let Some(Ok(name)) = reader.as_mut().unwrap().lines().nth(1) {
            set.insert(name);
        };
    }

    set
}

async fn cmd_output(mut cmd: Command) -> String {
    cmd.output()
        .await
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upgrades() {
        let upgrades = parse_upgrades(
            "kicad 10.0.6-2 -> 10.0.7-2\n\
             anyrun-git r314.55230c6-1 -> latest-commit\n\
             linux 6.1-1 -> 6.2-1 [ignored]\n\
             :: not an upgrade line\n",
        );

        assert_eq!(upgrades.len(), 3);
        assert_eq!(upgrades["kicad"].old, "10.0.6-2");
        assert_eq!(upgrades["kicad"].new, "10.0.7-2");
        assert_eq!(upgrades["anyrun-git"].new, "latest-commit");
        assert_eq!(upgrades["linux"].new, "6.2-1");
    }

    #[test]
    fn groups_upgradable_then_installed_then_rest() {
        let mut packages = FixedCompactStrings::with_capacity(64, 8);
        // Like the listing, several alphabetical runs: two repositories, then the AUR.
        for p in ["c", "inst2", "up2", "a", "inst1", "up1", "b", "up1"] {
            packages.push(p);
        }
        let installed: HashSet<String> = ["up1", "up2", "inst1", "inst2"]
            .into_iter()
            .map(String::from)
            .collect();
        let upgrades = parse_upgrades("up1 1 -> 2\nup2 3 -> 4\nnot-listed 5 -> 6\n");

        let grouped = group_packages(&packages, &installed, &upgrades);

        let order: Vec<&str> = grouped.packages.iter().collect();
        assert_eq!(
            order,
            // Installed groups sorted, the rest in listing order.
            ["up1", "up1", "up2", "inst1", "inst2", "c", "a", "b"]
        );
        assert_eq!(
            grouped.upgradable.keys().copied().collect::<IntSet<_>>(),
            [0, 1, 2].into_iter().collect()
        );
        assert_eq!(grouped.upgradable[&0].new, "2");
        assert_eq!(grouped.upgradable[&2].new, "4");
        assert_eq!(grouped.installed, [0, 1, 2, 3, 4].into_iter().collect());
    }
}
