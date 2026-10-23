use std::{collections::HashMap, env, fs, io, ops::Range, path::PathBuf};

/// Notes the user attached to packages, keyed by package name.
pub struct Notes {
    path: Option<PathBuf>,
    notes: HashMap<String, String>,
}

impl Notes {
    /// Loads the notes from `$XDG_DATA_HOME/parui/notes`, by default
    /// `~/.local/share/parui/notes`.
    pub fn load() -> Self {
        let path = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
            .map(|dir| dir.join("parui/notes"));

        let notes = path
            .as_ref()
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|content| parse(&content))
            .unwrap_or_default();

        Self { path, notes }
    }

    pub fn get(&self, package: &str) -> Option<&str> {
        self.notes.get(package).map(String::as_str)
    }

    pub fn contains(&self, package: &str) -> bool {
        self.notes.contains_key(package)
    }

    /// Sets the note of `package` and saves all notes. An empty note removes it.
    pub fn set(&mut self, package: &str, note: &str) -> io::Result<()> {
        let note = note.trim_end();
        if note.is_empty() {
            if self.notes.remove(package).is_none() {
                return Ok(());
            }
        } else if self.get(package) == Some(note) {
            return Ok(());
        } else {
            self.notes.insert(package.to_owned(), note.to_owned());
        }

        self.save()
    }

    fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Err(io::Error::other("neither XDG_DATA_HOME nor HOME is set"));
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        // Write next to the notes and rename over them, so they are never left half written.
        let staged = path.with_extension("tmp");
        fs::write(&staged, render(&self.notes))?;
        fs::rename(staged, path)
    }
}

/// Each line is a package name, a tab and its note, with `\`, newlines and tabs escaped.
fn parse(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .map(|(package, note)| (package.to_owned(), unescape(note)))
        .collect()
}

fn render(notes: &HashMap<String, String>) -> String {
    let mut packages: Vec<_> = notes.keys().collect();
    packages.sort_unstable();

    let mut out = String::from("# parui package notes: <package><TAB><note>\n");
    for package in packages {
        out.push_str(package);
        out.push('\t');
        out.push_str(&escape(&notes[package]));
        out.push('\n');
    }
    out
}

fn escape(note: &str) -> String {
    let mut out = String::with_capacity(note.len());
    for c in note.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn unescape(note: &str) -> String {
    let mut out = String::with_capacity(note.len());
    let mut chars = note.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(c) => out.push(c),
            None => out.push('\\'),
        }
    }
    out
}

/// Splits `text` into rows of at most `width` characters, breaking at newlines and wrapping long
/// lines at spaces. Every character is assumed to be one column wide.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows = Vec::new();
    for line in text.split('\n') {
        let chars: Vec<char> = line.chars().collect();
        rows.extend(
            row_ranges(&chars, width)
                .into_iter()
                .map(|range| chars[range].iter().collect()),
        );
    }
    rows
}

/// The rows of `line` wrapped to `width`, as ranges of character indices. A row ends at the last
/// space that fits, which belongs to no row, or mid-word if the word is longer than a row.
fn row_ranges(line: &[char], width: usize) -> Vec<Range<usize>> {
    let mut rows = Vec::new();
    let mut start = 0;
    while line.len() - start > width {
        // Include the first character past the row, as a space there ends the row exactly.
        match line[start..=start + width].iter().rposition(|&c| c == ' ') {
            Some(space) if space > 0 => {
                rows.push(start..start + space);
                start += space + 1;
            }
            _ => {
                rows.push(start..start + width);
                start += width;
            }
        }
    }
    rows.push(start..line.len());
    rows
}

/// A note being edited, with the cursor as a byte offset into the text.
pub struct NoteEditor {
    pub package: String,
    text: String,
    cursor: usize,
}

impl NoteEditor {
    pub fn new(package: String, text: &str) -> Self {
        Self {
            package,
            text: text.to_owned(),
            cursor: text.len(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn insert(&mut self, c: char) {
        if c == '\r' {
            return;
        }
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert(c);
        }
    }

    pub fn backspace(&mut self) {
        if let Some(c) = self.text[..self.cursor].chars().next_back() {
            self.cursor -= c.len_utf8();
            self.text.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        if let Some(c) = self.text[..self.cursor].chars().next_back() {
            self.cursor -= c.len_utf8();
        }
    }

    pub fn right(&mut self) {
        if let Some(c) = self.text[self.cursor..].chars().next() {
            self.cursor += c.len_utf8();
        }
    }

    fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1)
    }

    fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |i| self.cursor + i)
    }

    pub fn home(&mut self) {
        self.cursor = self.line_start();
    }

    pub fn end(&mut self) {
        self.cursor = self.line_end();
    }

    /// Moves to the same column of the previous line, or its end if it is shorter.
    pub fn up(&mut self) {
        let start = self.line_start();
        if start == 0 {
            self.cursor = 0;
            return;
        }
        let col = self.text[start..self.cursor].chars().count();
        let prev_start = self.text[..start - 1].rfind('\n').map_or(0, |i| i + 1);
        self.cursor = Self::offset_at(&self.text, prev_start, start - 1, col);
    }

    /// Moves to the same column of the next line, or its end if it is shorter.
    pub fn down(&mut self) {
        let end = self.line_end();
        if end == self.text.len() {
            self.cursor = end;
            return;
        }
        let col = self.text[self.line_start()..self.cursor].chars().count();
        let next_start = end + 1;
        let next_end = self.text[next_start..]
            .find('\n')
            .map_or(self.text.len(), |i| next_start + i);
        self.cursor = Self::offset_at(&self.text, next_start, next_end, col);
    }

    /// Byte offset of column `col` in the line `text[start..end]`, clamped to its end.
    fn offset_at(text: &str, start: usize, end: usize, col: usize) -> usize {
        text[start..end]
            .char_indices()
            .nth(col)
            .map_or(end, |(i, _)| start + i)
    }

    /// The cursor as a row and column of [`wrap`]`(self.text(), width)`.
    pub fn cursor_position(&self, width: usize) -> (usize, usize) {
        let width = width.max(1);
        let line_start = self.line_start();

        let mut row = 0;
        if line_start > 0 {
            for line in self.text[..line_start - 1].split('\n') {
                let chars: Vec<char> = line.chars().collect();
                row += row_ranges(&chars, width).len();
            }
        }

        let line: Vec<char> = self.text[line_start..self.line_end()].chars().collect();
        let col = self.text[line_start..self.cursor].chars().count();
        let ranges = row_ranges(&line, width);
        let (idx, range) = ranges
            .iter()
            .enumerate()
            .rfind(|(_, range)| range.start <= col)
            .expect("the first row starts at 0");
        (row + idx, col - range.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_notes() {
        let mut notes = HashMap::new();
        notes.insert(
            "kicad".to_owned(),
            "Held at 10.0.6\nsee \\n\tdone".to_owned(),
        );
        notes.insert("zen-browser-bin".to_owned(), "fine".to_owned());

        assert_eq!(parse(&render(&notes)), notes);
    }

    #[test]
    fn wraps_text() {
        assert_eq!(wrap("abcdef\n\nxy", 4), ["abcd", "ef", "", "xy"]);
        assert_eq!(wrap("", 4), [""]);
        assert_eq!(wrap("aa bb cc", 5), ["aa bb", "cc"]);
        assert_eq!(wrap("aa bbb", 4), ["aa", "bbb"]);
        assert_eq!(wrap("abcdefg h", 3), ["abc", "def", "g h"]);
    }

    #[test]
    fn tracks_cursor_across_word_wraps() {
        let mut editor = NoteEditor::new("p".to_owned(), "aa bbb\ncc");
        assert_eq!(editor.cursor_position(4), (2, 2));
        editor.up();
        // Column 2 of the first line is before the space ending the row "aa".
        assert_eq!(editor.cursor_position(4), (0, 2));
        editor.right();
        editor.right();
        assert_eq!(editor.cursor_position(4), (1, 1));
    }

    #[test]
    fn edits_and_tracks_cursor() {
        let mut editor = NoteEditor::new("p".to_owned(), "abcdef\nxy");
        assert_eq!(editor.cursor_position(4), (2, 2));

        editor.up();
        assert_eq!(editor.cursor_position(4), (0, 2));
        editor.end();
        assert_eq!(editor.cursor_position(4), (1, 2));
        editor.backspace();
        editor.insert('é');
        assert_eq!(editor.text(), "abcdeé\nxy");

        editor.down();
        assert_eq!(editor.cursor_position(4), (2, 2));
        editor.home();
        editor.insert_str("1\r\n");
        assert_eq!(editor.text(), "abcdeé\n1\nxy");
        assert_eq!(editor.cursor_position(4), (3, 0));

        editor.left();
        editor.delete();
        assert_eq!(editor.text(), "abcdeé\n1xy");
    }
}
