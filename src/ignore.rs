use std::{
    io::{self, Write},
    process::{Command, Stdio},
};

/// File managed by parui holding its `IgnorePkg` entries. pacman only reads it once it is
/// included from the `[options]` section of `/etc/pacman.conf`.
pub const IGNORE_FILE: &str = "/etc/pacman.d/parui-ignore.conf";

pub const PACMAN_CONF: &str = "/etc/pacman.conf";

/// The `IgnorePkg` patterns pacman effectively uses, from every configuration file.
pub struct Ignored {
    patterns: Vec<String>,
}

impl Ignored {
    pub fn load() -> Self {
        let patterns = Command::new("pacman-conf")
            .arg("IgnorePkg")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|out| out.split_whitespace().map(ToOwned::to_owned).collect())
            .unwrap_or_default();

        Self { patterns }
    }

    pub fn matches(&self, package: &str) -> bool {
        self.patterns
            .iter()
            .any(|pattern| glob_matches(pattern, package))
    }
}

/// Matches pacman's `IgnorePkg` globs, supporting `*` and `?`.
fn glob_matches(pattern: &str, name: &str) -> bool {
    let pattern = pattern.as_bytes();
    let name = name.as_bytes();

    let (mut p, mut n) = (0, 0);
    // Position of the last `*` in `pattern` and the position in `name` it currently matches up to.
    let mut star = None;

    while n < name.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some((p, n));
            p += 1;
        } else if let Some((star_p, star_n)) = star {
            p = star_p + 1;
            n = star_n + 1;
            star = Some((star_p, n));
        } else {
            return false;
        }
    }

    pattern[p..].iter().all(|&b| b == b'*')
}

/// Packages listed in [`IGNORE_FILE`], or none if it does not exist yet.
pub fn read_own() -> Vec<String> {
    std::fs::read_to_string(IGNORE_FILE)
        .map(|content| parse_own(&content))
        .unwrap_or_default()
}

fn parse_own(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("IgnorePkg")?
                .trim()
                .strip_prefix('=')
        })
        .flat_map(str::split_whitespace)
        .map(ToOwned::to_owned)
        .collect()
}

fn render_own(packages: &[String]) -> String {
    let mut content = format!(
        "# Managed by parui: packages skipped by pacman upgrades.\n\
         # Include it from the [options] section of /etc/pacman.conf with:\n\
         #   Include = {IGNORE_FILE}\n"
    );
    if !packages.is_empty() {
        content.push_str("IgnorePkg = ");
        content.push_str(&packages.join(" "));
        content.push('\n');
    }
    content
}

/// Writes `packages` to [`IGNORE_FILE`]. Without `interactive`, sudo fails instead of asking for
/// a password.
pub fn write_own(packages: &[String], interactive: bool) -> io::Result<bool> {
    sudo_write(IGNORE_FILE, &render_own(packages), interactive)
}

/// Whether the `[options]` section of [`PACMAN_CONF`] includes [`IGNORE_FILE`].
pub fn is_included() -> bool {
    std::fs::read_to_string(PACMAN_CONF).is_ok_and(|conf| includes_own(&conf))
}

fn includes_own(conf: &str) -> bool {
    let mut in_options = false;
    conf.lines().map(str::trim).any(|line| {
        if line.starts_with('[') {
            in_options = line == "[options]";
            return false;
        }
        in_options
            && line
                .strip_prefix("Include")
                .and_then(|rest| rest.trim().strip_prefix('='))
                .is_some_and(|path| path.trim() == IGNORE_FILE)
    })
}

/// `conf` with an `Include` of [`IGNORE_FILE`] right below its `[options]` header, or `None` if
/// it has no `[options]` section.
fn with_include(conf: &str) -> Option<String> {
    let mut out = String::with_capacity(conf.len() + 64);
    let mut inserted = false;
    for line in conf.split_inclusive('\n') {
        out.push_str(line);
        if !inserted && line.trim() == "[options]" {
            if !line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("# Added by parui\nInclude = {IGNORE_FILE}\n"));
            inserted = true;
        }
    }
    inserted.then_some(out)
}

/// Adds an `Include` of [`IGNORE_FILE`] to [`PACMAN_CONF`], asking sudo for a password if needed.
///
/// The new content is written next to it and renamed over it, so an interruption cannot leave a
/// truncated `pacman.conf` behind.
pub fn add_include() -> io::Result<bool> {
    let conf = std::fs::read_to_string(PACMAN_CONF)?;
    let Some(conf) = with_include(&conf) else {
        return Err(io::Error::other(format!(
            "{PACMAN_CONF} has no [options] section"
        )));
    };

    let staged = format!("{PACMAN_CONF}.parui-new");
    Ok(sudo_write(&staged, &conf, true)?
        && Command::new("sudo")
            .args(["mv", "-f", &staged, PACMAN_CONF])
            .status()?
            .success())
}

/// Writes `content` to `path` through `sudo tee`.
fn sudo_write(path: &str, content: &str, interactive: bool) -> io::Result<bool> {
    let mut cmd = Command::new("sudo");
    if !interactive {
        cmd.arg("-n");
    }
    cmd.args(["tee", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null());
    if !interactive {
        cmd.stderr(Stdio::null());
    }

    let mut child = cmd.spawn()?;
    // Dropping stdin after writing closes it, so tee sees the end of its input.
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(content.as_bytes())?;

    Ok(child.wait()?.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_globs() {
        assert!(glob_matches("kicad", "kicad"));
        assert!(!glob_matches("kicad", "kicad-library"));
        assert!(glob_matches("kicad*", "kicad-library"));
        assert!(glob_matches("*-git", "anyrun-git"));
        assert!(!glob_matches("*-git", "anyrun-gitx"));
        assert!(glob_matches("linux?", "linux6"));
        assert!(!glob_matches("linux?", "linux"));
        assert!(glob_matches("*a*b*", "xxaxxbxx"));
        assert!(glob_matches("*", ""));
    }

    #[test]
    fn round_trips_own_file() {
        let packages = vec!["kicad".to_owned(), "zen-browser-bin".to_owned()];
        assert_eq!(parse_own(&render_own(&packages)), packages);
        assert!(parse_own(&render_own(&[])).is_empty());
        assert_eq!(
            parse_own("IgnorePkg=a b\n  IgnorePkg   =  c\n#IgnorePkg = d\n"),
            ["a", "b", "c"]
        );
    }

    #[test]
    fn adds_include_to_options() {
        let conf = "# comment\n[options]\nHoldPkg = pacman\n\n[core]\nInclude = /etc/pacman.d/mirrorlist\n";
        assert!(!includes_own(conf));

        let with = with_include(conf).unwrap();
        assert!(includes_own(&with));
        assert_eq!(
            with,
            format!(
                "# comment\n[options]\n# Added by parui\nInclude = {IGNORE_FILE}\nHoldPkg = pacman\n\n[core]\nInclude = /etc/pacman.d/mirrorlist\n"
            )
        );

        assert!(with_include("[core]\n").is_none());
        // Only an Include in [options] counts.
        assert!(!includes_own(&format!(
            "[options]\n[core]\nInclude = {IGNORE_FILE}\n"
        )));
        assert!(!includes_own(&format!(
            "[options]\n#Include = {IGNORE_FILE}\n"
        )));
    }
}
