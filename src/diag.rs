// One JSON line per error, kept short so a model can read and act on it cheaply.

#[derive(Debug, Clone)]
pub struct Diag {
    pub kind: String,
    pub line: usize,
    pub col: usize,
    pub msg: String,
    pub fix: Option<String>,
}

impl Diag {
    pub fn new(kind: &str, line: usize, col: usize, msg: &str) -> Diag {
        Diag { kind: kind.into(), line, col, msg: msg.into(), fix: None }
    }

    pub fn fix_hint(mut self, fix: &str) -> Diag {
        self.fix = Some(fix.into());
        self
    }

    pub fn to_json(&self, file: &str) -> String {
        let mut s = format!(
            "{{\"e\":\"{}\",\"at\":\"{}:{}:{}\",\"msg\":\"{}\"",
            self.kind,
            esc(file),
            self.line,
            self.col,
            esc(&self.msg)
        );
        if let Some(f) = &self.fix {
            s.push_str(&format!(",\"fix\":\"{}\"", esc(f)));
        }
        s.push('}');
        s
    }
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
