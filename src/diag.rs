// One JSON line per error, kept short so a model can read and act on it cheaply.

#[derive(Debug, Clone)]
pub struct Diag {
    pub kind: String,
    pub line: usize,
    pub col: usize,
    pub msg: String,
    pub fix: Option<String>,
    // A machine-applicable fix: replace the first `old` on the error's line with `new`.
    pub edit: Option<(String, String)>,
    // The edit is close enough to be applied without asking (a typo of one or two letters).
    pub sure: bool,
}

impl Diag {
    pub fn new(kind: &str, line: usize, col: usize, msg: &str) -> Diag {
        Diag {
            kind: kind.into(),
            line,
            col,
            msg: msg.into(),
            fix: None,
            edit: None,
            sure: false,
        }
    }

    pub fn fix_hint(mut self, fix: &str) -> Diag {
        self.fix = Some(fix.into());
        self
    }

    pub fn with_edit(mut self, old: &str, new: &str) -> Diag {
        self.edit = Some((old.into(), new.into()));
        self
    }

    // Applies the edit to the source text, if the old text is on the error's line (as a whole word).
    pub fn apply(&self, src: &str) -> Option<String> {
        let (old, new) = self.edit.as_ref()?;
        let mut lines: Vec<String> = src.split('\n').map(String::from).collect();
        let l = lines.get_mut(self.line.checked_sub(1)?)?;
        let word = |c: Option<char>| c.map_or(false, |c| c.is_alphanumeric() || c == '_');
        let mut from = 0;
        while let Some(p) = l[from..].find(old.as_str()).map(|p| p + from) {
            let (before, after) = (l[..p].chars().last(), l[p + old.len()..].chars().next());
            let starts_word = old
                .chars()
                .next()
                .map_or(false, |c| c.is_alphanumeric() || c == '_');
            let ends_word = old
                .chars()
                .last()
                .map_or(false, |c| c.is_alphanumeric() || c == '_');
            if !(starts_word && word(before)) && !(ends_word && word(after)) {
                l.replace_range(p..p + old.len(), new);
                return Some(lines.join("\n"));
            }
            from = p + old.len();
        }
        None
    }

    // No file name, column or error kind: programs are one file, and the line is enough to find the mistake.
    pub fn to_json(&self) -> String {
        let (msg, fix) = public(&self.msg, self.fix.as_deref());
        let mut s = format!("{{\"line\":{},\"msg\":\"{}\"", self.line, esc(&msg));
        if let Some(f) = &fix {
            s.push_str(&format!(",\"fix\":\"{}\"", esc(f)));
        }
        s.push('}');
        s
    }
}

// Brevi's own helper names (ax_...) never reach the reader: `type(v)` is `ax_type_of(v)` inside the compiler.
// A message about some other internal name is about Brevi, not the program, and says so.
fn public(msg: &str, fix: Option<&str>) -> (String, Option<String>) {
    let named = |s: &str| s.replace("ax_type_of", "type");
    let (msg, fix) = (named(msg), fix.map(named));
    let internal = |s: &str| {
        s.match_indices("ax_")
            .any(|(i, _)| i == 0 || !s[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_'))
    };
    if internal(&msg) || fix.as_deref().is_some_and(internal) {
        if internal(&msg) {
            return (
                "Brevi cannot build this line yet (a gap in Brevi, not a mistake in the program)"
                    .into(),
                Some("write it another way, or run the file with python3".into()),
            );
        }
        return (msg, None);
    }
    (msg, fix)
}

pub fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}
