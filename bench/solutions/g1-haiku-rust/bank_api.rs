use std::io::{self, BufRead, Write};

// ---------- minimal JSON parser ----------

#[derive(Debug, Clone)]
enum J {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\r' | b'\n') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn lit(&mut self, word: &str) -> Option<()> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Some(())
        } else {
            None
        }
    }

    fn value(&mut self) -> Option<J> {
        self.ws();
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(J::Str),
            b't' => self.lit("true").map(|_| J::Bool(true)),
            b'f' => self.lit("false").map(|_| J::Bool(false)),
            b'n' => self.lit("null").map(|_| J::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }

    fn digits(&mut self) -> Option<usize> {
        let start = self.i;
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i > start {
            Some(self.i - start)
        } else {
            None
        }
    }

    fn number(&mut self) -> Option<J> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        self.digits()?;
        if self.peek() == Some(b'.') {
            self.i += 1;
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            self.digits()?;
        }
        Some(J::Num(String::from_utf8_lossy(&self.s[start..self.i]).into_owned()))
    }

    fn string(&mut self) -> Option<String> {
        // assumes current char is '"'
        self.i += 1;
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let c = self.peek()?;
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek()?;
                    self.i += 1;
                    match e {
                        b'"' => buf.push(b'"'),
                        b'\\' => buf.push(b'\\'),
                        b'/' => buf.push(b'/'),
                        b'b' => buf.push(8),
                        b'f' => buf.push(12),
                        b'n' => buf.push(b'\n'),
                        b'r' => buf.push(b'\r'),
                        b't' => buf.push(b'\t'),
                        b'u' => {
                            if self.i + 4 > self.s.len() {
                                return None;
                            }
                            let hex = std::str::from_utf8(&self.s[self.i..self.i + 4]).ok()?;
                            let code = u32::from_str_radix(hex, 16).ok()?;
                            self.i += 4;
                            let ch = char::from_u32(code).unwrap_or('\u{FFFD}');
                            let mut tmp = [0u8; 4];
                            buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                        }
                        _ => return None,
                    }
                }
                c if c < 0x20 => return None,
                c => buf.push(c),
            }
        }
        String::from_utf8(buf).ok()
    }

    fn array(&mut self) -> Option<J> {
        self.i += 1; // '['
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Some(J::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            match self.peek()? {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    return Some(J::Arr(items));
                }
                _ => return None,
            }
        }
    }

    fn object(&mut self) -> Option<J> {
        self.i += 1; // '{'
        let mut fields = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Some(J::Obj(fields));
        }
        loop {
            self.ws();
            if self.peek()? != b'"' {
                return None;
            }
            let key = self.string()?;
            self.ws();
            if self.peek()? != b':' {
                return None;
            }
            self.i += 1;
            let val = self.value()?;
            fields.push((key, val));
            self.ws();
            match self.peek()? {
                b',' => self.i += 1,
                b'}' => {
                    self.i += 1;
                    return Some(J::Obj(fields));
                }
                _ => return None,
            }
        }
    }
}

fn parse_json(s: &str) -> Option<J> {
    let mut p = P { s: s.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i == p.s.len() {
        Some(v)
    } else {
        None
    }
}

fn field<'a>(obj: &'a J, key: &str) -> Option<&'a J> {
    match obj {
        J::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

// Integer value of a JSON number (no fraction or exponent).
fn as_int(v: Option<&J>) -> Option<i64> {
    match v? {
        J::Num(s) if !s.contains(['.', 'e', 'E']) => s.parse::<i64>().ok(),
        _ => None,
    }
}

// ---------- bank ----------

struct Account {
    id: i64,
    owner: String,
    balance: i64,
}

struct Bank {
    accounts: Vec<Account>,
}

fn esc(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn acct_json(a: &Account) -> String {
    format!(
        "{{\"id\":{},\"owner\":{},\"balance\":{}}}",
        a.id,
        esc(&a.owner),
        a.balance
    )
}

fn err(status: u16, msg: &str) -> (u16, String) {
    (status, format!("{{\"error\":\"{}\"}}", msg))
}

// Positive integer amount, else None.
fn amount_of(body: &J) -> Option<i64> {
    as_int(field(body, "amount")).filter(|n| *n > 0)
}

impl Bank {
    fn find(&self, id: i64) -> Option<usize> {
        if id >= 1 && (id as usize) <= self.accounts.len() {
            Some((id - 1) as usize)
        } else {
            None
        }
    }

    fn create(&mut self, body: &J) -> (u16, String) {
        match field(body, "owner") {
            Some(J::Str(name)) if !name.is_empty() => {
                let id = self.accounts.len() as i64 + 1;
                let acct = Account {
                    id,
                    owner: name.clone(),
                    balance: 0,
                };
                let s = acct_json(&acct);
                self.accounts.push(acct);
                (201, s)
            }
            _ => err(400, "owner required"),
        }
    }

    fn deposit_or_withdraw(&mut self, id: i64, body: &J, withdraw: bool) -> (u16, String) {
        let idx = match self.find(id) {
            Some(i) => i,
            None => return err(404, "no such account"),
        };
        let amount = match amount_of(body) {
            Some(n) => n,
            None => return err(400, "bad amount"),
        };
        let acct = &mut self.accounts[idx];
        if withdraw {
            if amount > acct.balance {
                return err(409, "insufficient funds");
            }
            acct.balance -= amount;
        } else {
            match acct.balance.checked_add(amount) {
                Some(b) => acct.balance = b,
                None => return err(400, "bad amount"),
            }
        }
        (200, acct_json(acct))
    }

    fn transfer(&mut self, body: &J) -> (u16, String) {
        let from = self.find(as_int(field(body, "from")).unwrap_or(0));
        let to = self.find(as_int(field(body, "to")).unwrap_or(0));
        let (from, to) = match (from, to) {
            (Some(f), Some(t)) => (f, t),
            _ => return err(404, "no such account"),
        };
        if from == to {
            return err(400, "same account");
        }
        let amount = match amount_of(body) {
            Some(n) => n,
            None => return err(400, "bad amount"),
        };
        if self.accounts[from].balance < amount {
            return err(409, "insufficient funds");
        }
        self.accounts[from].balance -= amount;
        self.accounts[to].balance += amount;
        (
            200,
            format!(
                "{{\"from\":{},\"to\":{}}}",
                self.accounts[from].balance, self.accounts[to].balance
            ),
        )
    }

    fn list(&self) -> (u16, String) {
        let mut refs: Vec<&Account> = self.accounts.iter().collect();
        refs.sort_by(|a, b| b.balance.cmp(&a.balance).then(a.id.cmp(&b.id)));
        let parts: Vec<String> = refs.iter().map(|a| acct_json(a)).collect();
        (200, format!("[{}]", parts.join(",")))
    }
}

fn is_numeric(s: &str) -> Option<i64> {
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse::<i64>().ok()
    } else {
        None
    }
}

const NOT_FOUND: (u16, &str) = (404, "{\"error\":\"not found\"}");

fn handle(bank: &mut Bank, method: &str, path: &str, body_text: Option<&str>) -> (u16, String) {
    let segs: Vec<&str> = path.split('/').collect();

    // Route first.
    enum Route {
        Create,
        List,
        Get(i64),
        Deposit(i64),
        Withdraw(i64),
        Transfer,
    }
    let route = match (method, segs.as_slice()) {
        ("POST", ["", "accounts"]) => Route::Create,
        ("GET", ["", "accounts"]) => Route::List,
        ("GET", ["", "accounts", id]) => match is_numeric(id) {
            Some(n) => Route::Get(n),
            None => return (NOT_FOUND.0, NOT_FOUND.1.to_string()),
        },
        ("POST", ["", "accounts", id, "deposit"]) => match is_numeric(id) {
            Some(n) => Route::Deposit(n),
            None => return (NOT_FOUND.0, NOT_FOUND.1.to_string()),
        },
        ("POST", ["", "accounts", id, "withdraw"]) => match is_numeric(id) {
            Some(n) => Route::Withdraw(n),
            None => return (NOT_FOUND.0, NOT_FOUND.1.to_string()),
        },
        ("POST", ["", "transfer"]) => Route::Transfer,
        _ => return (NOT_FOUND.0, NOT_FOUND.1.to_string()),
    };

    // Then the JSON body (absent body counts as an empty object).
    let body = match body_text.map(|t| t.trim()).filter(|t| !t.is_empty()) {
        None => J::Obj(Vec::new()),
        Some(t) => match parse_json(t) {
            Some(v) => v,
            None => return err(400, "bad json"),
        },
    };

    match route {
        Route::Create => bank.create(&body),
        Route::List => bank.list(),
        Route::Get(id) => match bank.find(id) {
            Some(i) => (200, acct_json(&bank.accounts[i])),
            None => err(404, "no such account"),
        },
        Route::Deposit(id) => bank.deposit_or_withdraw(id, &body, false),
        Route::Withdraw(id) => bank.deposit_or_withdraw(id, &body, true),
        Route::Transfer => bank.transfer(&body),
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut bank = Bank {
        accounts: Vec::new(),
    };

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let (method, rest) = match line.split_once(' ') {
            Some((m, r)) => (m, r),
            None => (line, ""),
        };
        let (path, body) = match rest.split_once(' ') {
            Some((p, b)) => (p, Some(b)),
            None => (rest, None),
        };
        let (status, json) = handle(&mut bank, method, path, body);
        writeln!(out, "{} {}", status, json).unwrap();
    }
    out.flush().unwrap();
}
