use std::{
    io::{self, Write},
    process::{Command, Stdio},
};

/// File managed by parui holding its `IgnorePkg` entries. pacman only reads it once it is
/// included from the `[options]` section of `/etc/pacman.conf`.
pub const IGNORE_FILE: &str = "/etc/pacman.d/parui-ignore.conf";

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

/// Writes `packages` to [`IGNORE_FILE`] through `sudo tee`. Without `interactive`, sudo fails
/// instead of asking for a password.
pub fn write_own(packages: &[String], interactive: bool) -> io::Result<bool> {
    let mut cmd = Command::new("sudo");
    if !interactive {
        cmd.arg("-n");
    }
    cmd.args(["tee", IGNORE_FILE])
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
        .write_all(render_own(packages).as_bytes())?;

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
}
