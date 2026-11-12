
// ---------- zoneinfo: IANA time zones, after CPython's Lib/zoneinfo (_zoneinfo.py, _common.py, _tzpath.py) ----------
// A zone is read from the TZif files under TZPATH, as CPython reads them; the file access is in zoneglue.rs, pasted
// into a program that imports zoneinfo (ZONE_FS). It reads only regular files under the TZPATH folders, by keys
// checked as _tzpath._validate_tzfile_path checks them, so the time zone database needs no --sandbox grant.
// Zones are cached by key for the program's life (ZoneInfo(k) is ZoneInfo(k)), as CPython's weak cache keeps them.

pub struct ZoneOps {
    // the bytes of a regular file, None when there is none
    pub read: fn(&str) -> Option<Vec<u8>>,
    // whether the path is a regular file
    pub is_file: fn(&str) -> bool,
    // every regular file under a folder, as paths relative to it ("Europe/Paris"), and whether the folder exists
    pub walk: fn(&str) -> Option<Vec<String>>,
    // an environment variable (PYTHONTZPATH, TZ)
    pub env: fn(&str) -> Option<String>,
    // for tzlocal: whether a path exists (os.path.exists), is a symlink, and its os.path.realpath
    pub exists: fn(&str) -> bool,
    pub is_link: fn(&str) -> bool,
    pub realpath: fn(&str) -> String,
}
pub static ZONE_FS: std::sync::OnceLock<ZoneOps> = std::sync::OnceLock::new();
fn zone_ops() -> Result<&'static ZoneOps, E> {
    ZONE_FS.get().ok_or_else(|| exc("OSError", "brevi: this program was built without the time zone database".into()))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tti {
    pub off: i32,
    pub dst: i32,
    pub name: Rc<str>,
}
#[derive(Clone, Copy, Debug)]
enum ZRule {
    // Jn (julian: Feb 29 never counted) or n, at secs into the day
    Day { d: i64, julian: bool, secs: i64 },
    // Mm.w.d
    Cal { m: i64, w: i64, d: i64, secs: i64 },
}
#[derive(Clone, Debug)]
struct ZStr {
    std: Tti,
    dst: Tti,
    dst_diff: i64,
    start: ZRule,
    end: ZRule,
}
#[derive(Clone, Debug)]
enum ZAfter {
    T(Tti),
    S(ZStr),
}
pub struct Zone {
    pub key: String,
    trans_utc: Vec<i64>,
    trans_local: [Vec<i64>; 2],
    ttis: Vec<Tti>,
    before: Option<Tti>,
    after: ZAfter,
    pub fixed: bool,
}
pub struct ZoneReg {
    zones: Vec<Rc<Zone>>,
    by_key: HashMap<String, u32>,
    vals: Vec<V>,
}
thread_local! {
    static ZONES: RefCell<ZoneReg> = RefCell::new(ZoneReg { zones: vec![], by_key: HashMap::new(), vals: vec![] });
}
pub fn zone(i: u32) -> Rc<Zone> {
    ZONES.with(|z| z.borrow().zones[i as usize].clone())
}

const ZEPOCH_ORD: i64 = 719163;
fn zi_days_before_year(year: i64) -> i64 {
    let y = year - 1;
    y * 365 + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400) - ZEPOCH_ORD
}
fn zi_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}
impl ZRule {
    fn year_to_epoch(&self, year: i64) -> i64 {
        let days = zi_days_before_year(year);
        match *self {
            ZRule::Day { d, julian, secs } => {
                let d = if julian && d >= 59 && zi_leap(year) { d + 1 } else { d };
                (days + d) * 86400 + secs
            }
            ZRule::Cal { m, w, d, secs } => {
                const BEFORE: [i64; 13] = [-1, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
                const DIM: [i64; 13] = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
                // calendar.monthrange: the weekday (Monday 0) of the month's first day, and its length
                let first_ord = days + ZEPOCH_ORD + BEFORE[m as usize] + (m > 2 && zi_leap(year)) as i64 + 1;
                let first_day = (first_ord + 6) % 7;
                let dim = DIM[m as usize] + (m == 2 && zi_leap(year)) as i64;
                let mut md = (d - (first_day + 1)).rem_euclid(7) + 1;
                md += (w - 1) * 7;
                if md > dim {
                    md -= 7;
                }
                let ordinal = days + BEFORE[m as usize] + (m > 2 && zi_leap(year)) as i64 + md;
                ordinal * 86400 + secs
            }
        }
    }
}
impl ZStr {
    fn transitions(&self, year: i64) -> (i64, i64) {
        (self.start.year_to_epoch(year), self.end.year_to_epoch(year))
    }
    fn trans_info(&self, ts: i64, year: i64, fold: bool) -> &Tti {
        let (mut start, mut end) = self.transitions(year);
        if fold == (self.dst_diff >= 0) {
            end -= self.dst_diff;
        } else {
            start += self.dst_diff;
        }
        let isdst = if start < end { start <= ts && ts < end } else { !(end <= ts && ts < start) };
        if isdst {
            &self.dst
        } else {
            &self.std
        }
    }
    fn trans_info_fromutc(&self, ts: i64, year: i64) -> (&Tti, bool) {
        let (mut start, mut end) = self.transitions(year);
        start -= self.std.off as i64;
        end -= self.dst.off as i64;
        let isdst = if start < end { start <= ts && ts < end } else { !(end <= ts && ts < start) };
        let (a, b) = if self.dst_diff > 0 { (end, end + self.dst_diff) } else { (start, start - self.dst_diff) };
        (if isdst { &self.dst } else { &self.std }, a <= ts && ts < b)
    }
}
fn bisect_right(xs: &[i64], x: i64) -> usize {
    xs.partition_point(|&v| v <= x)
}
impl Zone {
    // _find_trans: the local time type at local seconds since 1970 (ts), the wall clock's year and fold
    pub fn find(&self, ts: i64, year: i64, fold: bool) -> Tti {
        let lt = &self.trans_local[fold as usize];
        if !lt.is_empty() && ts < lt[0] {
            return self.before.clone().unwrap_or_else(|| self.after_tti(ts, year, fold));
        }
        if lt.is_empty() || ts > lt[lt.len() - 1] {
            return self.after_tti(ts, year, fold);
        }
        self.ttis[bisect_right(lt, ts) - 1].clone()
    }
    fn after_tti(&self, ts: i64, year: i64, fold: bool) -> Tti {
        match &self.after {
            ZAfter::S(s) => s.trans_info(ts, year, fold).clone(),
            ZAfter::T(t) => t.clone(),
        }
    }
    // fromutc: the local time type at UTC seconds since 1970 (ts, in the UTC wall clock's year), and the fold
    pub fn fromutc(&self, ts: i64, year: i64) -> (Tti, bool) {
        let tu = &self.trans_utc;
        let n = tu.len();
        if n >= 1 && ts < tu[0] {
            return (self.before.clone().unwrap_or_else(|| self.ttis[0].clone()), false);
        }
        if n == 0 || ts > tu[n - 1] {
            match &self.after {
                ZAfter::S(s) => {
                    let (t, f) = s.trans_info_fromutc(ts, year);
                    return (t.clone(), f);
                }
                ZAfter::T(t) if n == 0 => return (t.clone(), false),
                _ => {}
            }
        }
        let idx = bisect_right(tu, ts);
        let (prev, tti) = if n > 1 && ts >= tu[1] {
            (self.ttis[idx - 2].clone(), self.ttis[idx - 1].clone())
        } else if ts > tu[n - 1] {
            (self.ttis[n - 1].clone(), self.after_tti(ts, year, false))
        } else {
            (self.before.clone().unwrap_or_else(|| self.ttis[0].clone()), self.ttis[0].clone())
        };
        let shift = (prev.off - tti.off) as i64;
        let fold = shift > ts - tu[idx - 1];
        (tti, fold)
    }
    // utcoffset(None) and friends: a zone with one offset for all time has it; others have none
    pub fn fixed_tti(&self) -> Option<Tti> {
        match (&self.after, self.fixed) {
            (ZAfter::T(t), true) => Some(t.clone()),
            _ => None,
        }
    }
}

// ----- reading a TZif file (_common.load_data) -----
fn zi_bad<T>(m: &str) -> Result<T, E> {
    val_err(m.into())
}
struct ZRead<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> ZRead<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], E> {
        if self.at + n > self.b.len() {
            // struct.unpack on a short read
            return val_err(format!("unpack requires a buffer of {n} bytes"));
        }
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    fn i32(&mut self) -> Result<i64, E> {
        let s = self.take(4)?;
        Ok(i32::from_be_bytes([s[0], s[1], s[2], s[3]]) as i64)
    }
    fn header(&mut self) -> Result<(u8, [i64; 6]), E> {
        if self.b.len() < self.at + 4 || &self.b[self.at..self.at + 4] != b"TZif" {
            return zi_bad("Invalid TZif file: magic not found");
        }
        self.at += 4;
        let v = self.take(1)?[0];
        let version = if v == 0 { 1 } else { (v as char).to_digit(10).map(|d| d as u8).ok_or_else(|| exc("ValueError", format!("invalid literal for int() with base 10: {:?}", [v])))? };
        self.take(15)?;
        let mut h = [0i64; 6];
        for x in h.iter_mut() {
            *x = self.i32()?;
        }
        Ok((version, h))
    }
}
fn zi_load(key: &str, data: &[u8]) -> Result<Zone, E> {
    let mut r = ZRead { b: data, at: 0 };
    let (mut version, mut h) = r.header()?;
    // isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt
    let mut tsize = 4;
    if version != 1 {
        tsize = 8;
        let skip = h[3] * 5 + h[4] * 6 + h[5] + h[2] * 8 + h[1] + h[0];
        r.take(skip.max(0) as usize)?;
        let (v2, h2) = r.header()?;
        version = v2;
        h = h2;
    }
    let (timecnt, typecnt, charcnt) = (h[3] as usize, h[4] as usize, h[5] as usize);
    let mut trans_utc = Vec::with_capacity(timecnt);
    for _ in 0..timecnt {
        let s = r.take(tsize)?;
        trans_utc.push(if tsize == 8 { i64::from_be_bytes(s.try_into().unwrap()) } else { i32::from_be_bytes(s.try_into().unwrap()) as i64 });
    }
    let trans_idx: Vec<usize> = r.take(timecnt)?.iter().map(|&b| b as usize).collect();
    if let Some(&mx) = trans_idx.iter().max() {
        if mx >= typecnt {
            return val_err(format!("Invalid transition index found while reading TZif: {mx}"));
        }
    }
    let mut utcoff = vec![];
    let mut isdst = vec![];
    let mut abbrind = vec![];
    for _ in 0..typecnt {
        utcoff.push(r.i32()?);
        let s = r.take(2)?;
        isdst.push(s[0] as i8 != 0);
        abbrind.push(s[1] as i8 as i64);
    }
    let chars = r.take(charcnt)?.to_vec();
    let abbr: Vec<Rc<str>> = abbrind
        .iter()
        .map(|&i| {
            let i = if i < 0 { (chars.len() as i64 + i).max(0) as usize } else { (i as usize).min(chars.len()) };
            let end = chars[i..].iter().position(|&c| c == 0).map_or(chars.len() - 1, |p| i + p);
            Rc::from(String::from_utf8_lossy(&chars[i..end.max(i)]).as_ref())
        })
        .collect();
    let mut tz_str: Option<String> = None;
    if version >= 2 {
        r.take((h[0] + h[1] + h[2] * 12).max(0) as usize)?;
        let c = r.take(1)?;
        if c != b"\n" {
            return Err(exc("AssertionError", format!("{:?}", String::from_utf8_lossy(c))));
        }
        let rest = &r.b[r.at..];
        match rest.iter().position(|&c| c == b'\n') {
            Some(p) => tz_str = Some(String::from_utf8_lossy(&rest[..p]).into_owned()),
            None => return zi_bad("Invalid TZif file: unexpected end of file"),
        }
    }
    // _utcoff_to_dstoff
    let mut dstoffs = vec![0i64; typecnt];
    let dst_cnt = isdst.iter().filter(|&&d| d).count();
    let mut dst_found = 0;
    let mut broke = false;
    for i in 1..trans_idx.len() {
        if dst_cnt == dst_found {
            broke = true;
            break;
        }
        let idx = trans_idx[i];
        if !isdst[idx] || dstoffs[idx] != 0 {
            continue;
        }
        let mut dstoff = 0;
        let uo = utcoff[idx];
        let mut comp = trans_idx[i - 1];
        if !isdst[comp] {
            dstoff = uo - utcoff[comp];
        }
        if dstoff == 0 && idx < typecnt - 1 && i + 1 < trans_idx.len() {
            comp = trans_idx[i + 1];
            if isdst[comp] {
                continue;
            }
            dstoff = uo - utcoff[comp];
        }
        if dstoff != 0 {
            dst_found += 1;
            dstoffs[idx] = dstoff;
        }
    }
    if !broke {
        for idx in 0..typecnt {
            if dstoffs[idx] == 0 && isdst[idx] {
                dstoffs[idx] = 3600;
            }
        }
    }
    // _ts_to_local
    let mut trans_local = [trans_utc.clone(), trans_utc.clone()];
    if !trans_utc.is_empty() {
        let (mut o0, mut o1) = if utcoff.len() > 1 { (utcoff[0], utcoff[trans_idx[0]]) } else { (utcoff[0], utcoff[0]) };
        if o1 > o0 {
            std::mem::swap(&mut o0, &mut o1);
        }
        trans_local[0][0] += o0;
        trans_local[1][0] += o1;
        for i in 1..trans_idx.len() {
            let (mut o0, mut o1) = (utcoff[trans_idx[i - 1]], utcoff[trans_idx[i]]);
            if o1 > o0 {
                std::mem::swap(&mut o0, &mut o1);
            }
            trans_local[0][i] += o0;
            trans_local[1][i] += o1;
        }
    }
    let list: Vec<Tti> = (0..typecnt).map(|i| Tti { off: utcoff[i] as i32, dst: dstoffs[i] as i32, name: abbr[i].clone() }).collect();
    let ttis: Vec<Tti> = trans_idx.iter().map(|&i| list[i].clone()).collect();
    let before = (0..typecnt).find(|&i| !isdst[i]).map(|i| list[i].clone()).or_else(|| ttis.first().cloned());
    let after = match tz_str.filter(|s| !s.is_empty()) {
        Some(s) => zi_parse_tz_str(&s)?,
        None => {
            if ttis.is_empty() && list.is_empty() {
                return zi_bad("No time zone information found.");
            }
            ZAfter::T(ttis.last().or(list.last()).cloned().unwrap())
        }
    };
    let fixed = if list.len() > 1 || matches!(after, ZAfter::S(_)) {
        false
    } else if list.is_empty() {
        true
    } else {
        matches!(&after, ZAfter::T(t) if *t == list[0])
    };
    Ok(Zone { key: key.to_string(), trans_utc, trans_local, ttis, before, after, fixed })
}

// ----- the TZ string after the last transition (_parse_tz_str) -----
fn zi_abbr(s: &[u8], i: &mut usize) -> Option<String> {
    let st = *i;
    if s.get(*i) == Some(&b'<') {
        let mut j = *i + 1;
        while j < s.len() && (s[j].is_ascii_alphanumeric() || s[j] == b'+' || s[j] == b'-') {
            j += 1;
        }
        if j == *i + 1 || s.get(j) != Some(&b'>') {
            return None;
        }
        *i = j + 1;
        return Some(String::from_utf8_lossy(&s[st + 1..j]).into_owned());
    }
    while *i < s.len() && s[*i].is_ascii_alphabetic() {
        *i += 1;
    }
    (*i > st).then(|| String::from_utf8_lossy(&s[st..*i]).into_owned())
}
// [+-]?\d{1,3}(:\d{2}(:\d{2})?)? at s[i..]: (sign, h, m, s) and the end
fn zi_hms(s: &[u8], i: &mut usize) -> Option<(bool, i64, i64, i64)> {
    let mut j = *i;
    let neg = match s.get(j) {
        Some(b'-') => {
            j += 1;
            true
        }
        Some(b'+') => {
            j += 1;
            false
        }
        _ => false,
    };
    let d0 = j;
    while j < s.len() && j - d0 < 3 && s[j].is_ascii_digit() {
        j += 1;
    }
    if j == d0 {
        return None;
    }
    let h: i64 = std::str::from_utf8(&s[d0..j]).ok()?.parse().ok()?;
    let two = |j: usize| -> Option<i64> {
        if j + 3 <= s.len() && s[j] == b':' && s[j + 1].is_ascii_digit() && s[j + 2].is_ascii_digit() {
            Some(((s[j + 1] - b'0') * 10 + (s[j + 2] - b'0')) as i64)
        } else {
            None
        }
    };
    let (mut m, mut sec) = (0, 0);
    if let Some(x) = two(j) {
        m = x;
        j += 3;
        if let Some(y) = two(j) {
            sec = y;
            j += 3;
        }
    }
    *i = j;
    Some((neg, h, m, sec))
}
fn zi_parse_tz_str(tz: &str) -> Result<ZAfter, E> {
    let (off_str, rules) = match tz.split_once(',') {
        Some((a, b)) => (a, Some(b)),
        None => (tz, None),
    };
    let bad = || val_err::<ZAfter>(format!("{tz} is not a valid TZ string"));
    let s = off_str.as_bytes();
    let mut i = 0;
    let Some(std_abbr) = zi_abbr(s, &mut i) else { return bad() };
    let mut std_off = None;
    let mut dst_abbr = None;
    let mut dst_off = None;
    if i < s.len() {
        let Some(o) = zi_hms(s, &mut i) else { return bad() };
        std_off = Some(o);
        if i < s.len() {
            let Some(a) = zi_abbr(s, &mut i) else { return bad() };
            dst_abbr = Some(a);
            if i < s.len() {
                let Some(o) = zi_hms(s, &mut i) else { return bad() };
                dst_off = Some(o);
            }
        }
        if i != s.len() {
            return bad();
        }
    }
    let delta = |(neg, h, m, sec): (bool, i64, i64, i64)| -> Result<i64, ()> {
        if h > 24 {
            return Err(());
        }
        let t = h * 3600 + m * 60 + sec;
        Ok(if neg { t } else { -t })
    };
    let Some(so) = std_off else { return val_err(format!("Invalid STD offset in {tz}")) };
    let std_offset = delta(so).or_else(|_| val_err(format!("Invalid STD offset in {tz}")))?;
    match dst_abbr {
        Some(da) => {
            let dst_offset = match dst_off {
                Some(o) => delta(o).or_else(|_| val_err(format!("Invalid DST offset in {tz}")))?,
                None => std_offset + 3600,
            };
            let Some(rules) = rules else { return val_err(format!("Missing transition rules: {tz}")) };
            let (a, b) = rules.split_once(',').unwrap_or((rules, ""));
            let parse = |x: &str| zi_rule(x).ok_or(());
            let (start, end) = match (parse(a), if rules.contains(',') { parse(b) } else { Err(()) }) {
                (Ok(x), Ok(y)) => (x, y),
                _ => return val_err(format!("Invalid TZ string: {tz}")),
            };
            let dst_diff = dst_offset - std_offset;
            Ok(ZAfter::S(ZStr {
                std: Tti { off: std_offset as i32, dst: 0, name: Rc::from(std_abbr.as_str()) },
                dst: Tti { off: dst_offset as i32, dst: dst_diff as i32, name: Rc::from(da.as_str()) },
                dst_diff,
                start,
                end,
            }))
        }
        None if rules.is_some() => val_err(format!("Transition rule present without DST: {tz}")),
        None => Ok(ZAfter::T(Tti { off: std_offset as i32, dst: 0, name: Rc::from(std_abbr.as_str()) })),
    }
}
fn zi_rule(x: &str) -> Option<ZRule> {
    let (date, time) = match x.split_once('/') {
        Some((d, t)) => (d, Some(t)),
        None => (x, None),
    };
    let secs = match time {
        Some(t) => {
            let b = t.as_bytes();
            let mut i = 0;
            let (neg, h, m, s) = zi_hms(b, &mut i)?;
            if i != b.len() || h > 167 {
                return None;
            }
            let v = h * 3600 + m * 60 + s;
            if neg {
                -v
            } else {
                v
            }
        }
        None => 7200,
    };
    let digits = |s: &str, lo: usize, hi: usize| s.len() >= lo && s.len() <= hi && s.bytes().all(|c| c.is_ascii_digit());
    if let Some(r) = date.strip_prefix('M') {
        let p: Vec<&str> = r.split('.').collect();
        if p.len() != 3 || !digits(p[0], 1, 2) || !digits(p[1], 1, 1) || !digits(p[2], 1, 1) {
            return None;
        }
        let (m, w, d): (i64, i64, i64) = (p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?);
        if !(1..=12).contains(&m) || !(1..=5).contains(&w) || !(0..=6).contains(&d) {
            return None;
        }
        return Some(ZRule::Cal { m, w, d, secs });
    }
    let (julian, n) = match date.strip_prefix('J') {
        Some(n) => (true, n),
        None => (false, date),
    };
    if !digits(n, 1, 3) {
        return None;
    }
    let d: i64 = n.parse().ok()?;
    if d < julian as i64 || d > 365 {
        return None;
    }
    Some(ZRule::Day { d, julian, secs })
}

// ----- TZPATH and keys (_tzpath) -----
const ZONE_TZPATH: &str = "/usr/share/zoneinfo:/usr/lib/zoneinfo:/usr/share/lib/zoneinfo:/etc/zoneinfo";
pub fn tzpath() -> Vec<String> {
    let env = ZONE_FS.get().and_then(|o| (o.env)("PYTHONTZPATH"));
    let raw = env.unwrap_or_else(|| ZONE_TZPATH.to_string());
    if raw.is_empty() {
        return vec![];
    }
    raw.split(':').filter(|p| p.starts_with('/')).map(|p| p.to_string()).collect()
}
// posixpath.normpath
fn zi_normpath(p: &str) -> String {
    if p.is_empty() {
        return ".".into();
    }
    let lead = if p.starts_with("//") && !p.starts_with("///") { 2 } else if p.starts_with('/') { 1 } else { 0 };
    let mut out: Vec<&str> = vec![];
    for c in p.split('/') {
        if c.is_empty() || c == "." {
            continue;
        }
        if c != ".." || (lead == 0 && out.is_empty()) || out.last() == Some(&"..") {
            out.push(c);
        } else if !out.is_empty() {
            out.pop();
        }
    }
    let s = "/".repeat(lead) + &out.join("/");
    if s.is_empty() {
        ".".into()
    } else {
        s
    }
}
fn zi_validate(key: &str) -> Result<(), E> {
    if key.starts_with('/') {
        return val_err(format!("ZoneInfo keys may not be absolute paths, got: {key}"));
    }
    let n = zi_normpath(key);
    if n.len() != key.len() {
        return val_err(format!("ZoneInfo keys must be normalized relative paths, got: {key}"));
    }
    if !zi_normpath(&format!("_/{n}")).starts_with("_/") {
        return val_err(format!("ZoneInfo keys must refer to subdirectories of TZPATH, got: {key}"));
    }
    Ok(())
}
fn zi_not_found(key: &str) -> E {
    exc("ZoneInfoNotFoundError", format!("No time zone found with key {key}"))
}
// ZoneInfo(key): the cached zone value
pub fn zoneinfo_new(mut a: Args) -> R {
    let k = match a.kw("key").or_else(|| a.p.first().cloned()) {
        Some(k) => k,
        None => return type_err("ZoneInfo.__new__() missing required argument 'key' (pos 1)".into()),
    };
    let key = match &k {
        V::Str(x) => x.text().to_string(),
        x => return type_err(format!("expected str, bytes or os.PathLike object, not {}", tn(x))),
    };
    if let Some(v) = ZONES.with(|z| z.borrow().by_key.get(&key).map(|&i| z.borrow().vals[i as usize].clone())) {
        return Ok(v);
    }
    zi_validate(&key)?;
    let ops = zone_ops()?;
    let mut data = None;
    for p in tzpath() {
        let f = format!("{p}/{key}");
        if (ops.is_file)(&f) {
            data = (ops.read)(&f);
            if data.is_none() {
                return Err(exc("OSError", format!("could not read {f}")));
            }
            break;
        }
    }
    let Some(data) = data else { return Err(zi_not_found(&key)) };
    let z = zi_load(&key, &data)?;
    Ok(ZONES.with(|r| {
        let mut r = r.borrow_mut();
        let i = r.zones.len() as u32;
        r.zones.push(Rc::new(z));
        let v = V::Ext(ExtRc::new(ZIV(i)));
        r.vals.push(v.clone());
        r.by_key.insert(key, i);
        v
    }))
}
pub fn available_timezones() -> R {
    let ops = zone_ops()?;
    let set = set_new();
    let mut keys: Vec<String> = vec![];
    for root in tzpath() {
        let Some(files) = (ops.walk)(&root) else { continue };
        for k in files {
            if k.starts_with("right/") || k.starts_with("posix/") || k.is_empty() || keys.contains(&k) {
                continue;
            }
            let p = format!("{root}/{k}");
            if (ops.read)(&p).is_some_and(|b| b.starts_with(b"TZif")) {
                keys.push(k);
            }
        }
    }
    if let V::Set(s) = &set {
        let mut s = s.borrow_mut();
        for k in keys {
            if k != "posixrules" {
                s.add(st(k))?;
            }
        }
    }
    Ok(set)
}

// A zoneinfo.ZoneInfo value: an index into ZONES.
pub struct ZIV(pub u32);
pub fn zi_of(v: &V) -> Option<u32> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<ZIV>().map(|z| z.0),
        _ => None,
    }
}
impl Ext for ZIV {
    fn tname(&self) -> &'static str {
        "zoneinfo.ZoneInfo"
    }
    fn repr(&self) -> String {
        format!("zoneinfo.ZoneInfo(key={})", str_repr(&zone(self.0).key))
    }
    fn str_(&self) -> String {
        zone(self.0).key.clone()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some(hash_int(self.0 as i64 * 7919 + 1))
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(zi_of(o) == Some(self.0))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        match n {
            "key" => Some(Ok(st(zone(self.0).key.clone()))),
            _ => None,
        }
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        let z = zone(self.0);
        let arg = a.p.first().cloned().unwrap_or(V::None);
        let tti = || -> Result<Option<Tti>, E> {
            match &arg {
                V::None => Ok(z.fixed_tti()),
                v => match dt_zone_tti(v, self.0) {
                    Some(t) => Ok(Some(t)),
                    None => type_err(format!("{n}() argument must be a datetime.datetime or None, not {}", tn(v))),
                },
            }
        };
        Some((|| -> R {
            Ok(match n {
                "utcoffset" => tti()?.map_or(V::None, |t| td_secs(t.off as i64)),
                "dst" => tti()?.map_or(V::None, |t| td_secs(t.dst as i64)),
                "tzname" => tti()?.map_or(V::None, |t| st(t.name.to_string())),
                "fromutc" => dt_fromutc(me, &arg)?,
                _ => return Err(exc("AttributeError", format!("'zoneinfo.ZoneInfo' object has no attribute '{n}'"))),
            })
        })())
    }
}
// ----- tzlocal's get_localzone_name on Linux and macOS (tzlocal 5.4 unix.py), over the same file access -----
fn tzl_text(ops: &ZoneOps, p: &str) -> Option<String> {
    (ops.read)(p).and_then(|b| String::from_utf8(b).ok())
}
fn tzl_name_from_env(ops: &ZoneOps) -> Option<String> {
    let mut t = (ops.env)("TZ").filter(|t| !t.is_empty())?;
    if t.starts_with(':') {
        t.remove(0);
    }
    if TZ_WIN.contains(&t.as_str()) {
        return Some(t);
    }
    if t.starts_with('/') && (ops.exists)(&t) {
        let parts: Vec<String> = (ops.realpath)(&t).split('/').map(|x| x.to_string()).collect();
        for start in 0..parts.len() {
            let p = parts[start..].join("/");
            if TZ_WIN.contains(&p.as_str()) {
                return Some(p);
            }
        }
    }
    None
}
fn tzl_unique(ops: &ZoneOps, found: &[(String, String)]) -> Vec<String> {
    let mut u: Vec<String> = vec![];
    for (_, name) in found {
        let p = (ops.realpath)(&format!("/usr/share/zoneinfo/{name}"));
        let real = p.split('/').skip(4).collect::<Vec<_>>().join("/");
        if !u.contains(&real) {
            u.push(real);
        }
    }
    u
}
pub fn tzlocal_name() -> R {
    let ops = zone_ops()?;
    if let Some(t) = tzl_name_from_env(ops) {
        return Ok(st(t));
    }
    if (ops.exists)("/system/bin/getprop") {
        return Err(unsupported("tzlocal on Termux (getprop)"));
    }
    // (config file, zone) in the order tzlocal's dict keeps them
    let mut found: Vec<(String, String)> = vec![];
    let mut put = |found: &mut Vec<(String, String)>, k: String, v: String| match found.iter_mut().find(|(a, _)| *a == k) {
        Some(e) => e.1 = v,
        None => found.push((k, v)),
    };
    for f in ["/etc/timezone", "/var/db/zoneinfo"] {
        let Some(data) = tzl_text(ops, f).filter(|d| d.is_ascii()) else { continue };
        let lines = data.trim_matches(|c| matches!(c, '/' | ' ' | '\t' | '\r' | '\n'));
        if lines.is_empty() {
            continue;
        }
        for l in lines.lines() {
            let mut e = l;
            if let Some((a, _)) = e.split_once(' ') {
                e = a;
            }
            if let Some((a, _)) = e.split_once('#') {
                e = a;
            }
            if !e.is_empty() {
                put(&mut found, f.to_string(), e.replace(' ', "_"));
            }
        }
    }
    for f in ["/etc/sysconfig/clock", "/etc/conf.d/clock"] {
        let Some(data) = tzl_text(ops, f) else { continue };
        for line in data.split_inclusive('\n') {
            let t = line.trim_start();
            let after = |k: &str| -> Option<&str> {
                let r = t.strip_prefix(k)?.trim_start().strip_prefix('=')?.trim_start();
                r.strip_prefix('"')
            };
            if let Some(rest) = after("ZONE").or_else(|| after("TIMEZONE")) {
                match rest.find('"') {
                    Some(e) => put(&mut found, f.to_string(), rest[..e].replace(' ', "_")),
                    None => eprintln!("UserWarning: Syntax error in {f}. Ignoring line: {line}"),
                }
            }
        }
    }
    if (ops.exists)("/etc/localtime") && (ops.is_link)("/etc/localtime") {
        let mut etctz = (ops.realpath)("/etc/localtime");
        let mut start = etctz.find('/').map_or(0, |i| i + 1);
        while start != 0 {
            etctz = etctz[start..].to_string();
            match zoneinfo_new(Args::p(vec![st(etctz.clone())])) {
                Ok(_) => {
                    put(&mut found, "/etc/localtime is a symlink to".into(), etctz.replace(' ', "_"));
                    break;
                }
                Err(e) if zi_is_exc(&e, "ZoneInfoNotFoundError") => {}
                Err(e) => return Err(e),
            }
            start = etctz.find('/').map_or(0, |i| i + 1);
        }
    }
    if found.is_empty() {
        return Ok(V::None);
    }
    if found.len() > 1 {
        let mut u = tzl_unique(ops, &found);
        if u.len() != 1 && found.iter().any(|(k, _)| k.contains("etc/timezone")) {
            eprintln!("/etc/timezone is deprecated in some distros, and no longer reliable. tzlocal is ignoring it, and you can likely delete it.");
            found.retain(|(k, _)| !k.contains("etc/timezone"));
            u = tzl_unique(ops, &found);
        }
        if u.len() != 1 {
            let mut m = String::from("Multiple conflicting time zone configurations found:\n");
            for (k, v) in &found {
                m.push_str(&format!("{k}: {v}\n"));
            }
            m.push_str("Fix the configuration, or set the time zone in a TZ environment variable.\n");
            return Err(exc("ZoneInfoNotFoundError", m));
        }
    }
    Ok(st(found[0].1.clone()))
}
fn zi_is_exc(e: &E, name: &str) -> bool {
    matches!(&e.0, V::Obj(o) if &*o.cls.name == name)
}
// tzlocal's utils._tz_from_env: TZ names a file or a zone, or get_localzone refuses it
fn tzl_from_env() -> Result<Option<V>, E> {
    let ops = zone_ops()?;
    let Some(mut t) = (ops.env)("TZ").filter(|t| !t.is_empty()) else { return Ok(None) };
    if t.starts_with(':') {
        t.remove(0);
    }
    if t.starts_with('/') && (ops.exists)(&t) {
        let name = tzl_name_from_env(ops).unwrap_or_else(|| t.rsplit('/').next().unwrap_or("").to_string());
        let Some(data) = (ops.read)(&t) else { return Err(exc("OSError", format!("could not read {t}"))) };
        return zi_uncached(&name, &data).map(Some);
    }
    match zoneinfo_new(Args::p(vec![st(t.clone())])) {
        Ok(z) => Ok(Some(z)),
        Err(e) if zi_is_exc(&e, "ZoneInfoNotFoundError") => Err(exc(
            "ZoneInfoNotFoundError",
            format!("tzlocal() does not support non-zoneinfo timezones like {t}. \nPlease use a timezone in the form of Continent/City"),
        )),
        Err(e) => Err(e),
    }
}
// ZoneInfo.from_file(f, key=...): a zone object outside the key cache
fn zi_uncached(key: &str, data: &[u8]) -> R {
    let z = zi_load(key, data)?;
    Ok(ZONES.with(|r| {
        let mut r = r.borrow_mut();
        let i = r.zones.len() as u32;
        r.zones.push(Rc::new(z));
        let v = V::Ext(ExtRc::new(ZIV(i)));
        r.vals.push(v.clone());
        v
    }))
}
pub fn tzlocal_fn(n: &str, _a: Args) -> R {
    match n {
        "get_localzone_name" => tzlocal_name(),
        "get_localzone" => {
            if let Some(z) = tzl_from_env()? {
                return Ok(z);
            }
            match tzlocal_name()? {
                V::None => match (zone_ops()?.read)("/etc/localtime") {
                    Some(data) => zi_uncached("local", &data),
                    None => Err(unsupported("tzlocal.get_localzone without a configured zone")),
                },
                k => zoneinfo_new(Args::p(vec![k])),
            }
        }
        _ => Err(unsupported(&format!("tzlocal.{n}"))),
    }
}
pub fn zoneinfo_attr(n: &str) -> Option<V> {
    Some(match n {
        "ZoneInfo" => V::Class(ext_class("zoneinfo.ZoneInfo")),
        "ZoneInfoNotFoundError" => V::Class(exc_class("ZoneInfoNotFoundError")),
        "TZPATH" => tup(tzpath().into_iter().map(st).collect()),
        _ => return None,
    })
}
pub fn zoneinfo_fn(n: &str, a: Args) -> R {
    match n {
        "available_timezones" => available_timezones(),
        "reset_tzpath" if a.p.is_empty() && a.k.is_empty() => Ok(V::None),
        _ => Err(unsupported(&format!("zoneinfo.{n}"))),
    }
}

// tzlocal's windows_tz.tz_win keys (tzlocal 5.4.5, MIT): the names tzlocal accepts from the TZ environment variable.
const TZ_WIN: &[&str] = &[
    "", "Africa/Abidjan", "Africa/Accra", "Africa/Addis_Ababa", "Africa/Algiers", "Africa/Asmara", "Africa/Asmera",
    "Africa/Bamako", "Africa/Bangui", "Africa/Banjul", "Africa/Bissau", "Africa/Blantyre", "Africa/Brazzaville",
    "Africa/Bujumbura", "Africa/Cairo", "Africa/Casablanca", "Africa/Ceuta", "Africa/Conakry", "Africa/Dakar",
    "Africa/Dar_es_Salaam", "Africa/Djibouti", "Africa/Douala", "Africa/El_Aaiun", "Africa/Freetown",
    "Africa/Gaborone", "Africa/Harare", "Africa/Johannesburg", "Africa/Juba", "Africa/Kampala", "Africa/Khartoum",
    "Africa/Kigali", "Africa/Kinshasa", "Africa/Lagos", "Africa/Libreville", "Africa/Lome", "Africa/Luanda",
    "Africa/Lubumbashi", "Africa/Lusaka", "Africa/Malabo", "Africa/Maputo", "Africa/Maseru", "Africa/Mbabane",
    "Africa/Mogadishu", "Africa/Monrovia", "Africa/Nairobi", "Africa/Ndjamena", "Africa/Niamey", "Africa/Nouakchott",
    "Africa/Ouagadougou", "Africa/Porto-Novo", "Africa/Sao_Tome", "Africa/Timbuktu", "Africa/Tripoli",
    "Africa/Tunis", "Africa/Windhoek", "America/Adak", "America/Anchorage", "America/Anguilla", "America/Antigua",
    "America/Araguaina", "America/Argentina/Buenos_Aires", "America/Argentina/Catamarca",
    "America/Argentina/ComodRivadavia", "America/Argentina/Cordoba", "America/Argentina/Jujuy",
    "America/Argentina/La_Rioja", "America/Argentina/Mendoza", "America/Argentina/Rio_Gallegos",
    "America/Argentina/Salta", "America/Argentina/San_Juan", "America/Argentina/San_Luis",
    "America/Argentina/Tucuman", "America/Argentina/Ushuaia", "America/Aruba", "America/Asuncion",
    "America/Atikokan", "America/Atka", "America/Bahia", "America/Bahia_Banderas", "America/Barbados",
    "America/Belem", "America/Belize", "America/Blanc-Sablon", "America/Boa_Vista", "America/Bogota",
    "America/Boise", "America/Buenos_Aires", "America/Cambridge_Bay", "America/Campo_Grande", "America/Cancun",
    "America/Caracas", "America/Catamarca", "America/Cayenne", "America/Cayman", "America/Chicago",
    "America/Chihuahua", "America/Ciudad_Juarez", "America/Coral_Harbour", "America/Cordoba", "America/Costa_Rica",
    "America/Coyhaique", "America/Creston", "America/Cuiaba", "America/Curacao", "America/Danmarkshavn",
    "America/Dawson", "America/Dawson_Creek", "America/Denver", "America/Detroit", "America/Dominica",
    "America/Edmonton", "America/Eirunepe", "America/El_Salvador", "America/Ensenada", "America/Fort_Nelson",
    "America/Fort_Wayne", "America/Fortaleza", "America/Glace_Bay", "America/Godthab", "America/Goose_Bay",
    "America/Grand_Turk", "America/Grenada", "America/Guadeloupe", "America/Guatemala", "America/Guayaquil",
    "America/Guyana", "America/Halifax", "America/Havana", "America/Hermosillo", "America/Indiana/Indianapolis",
    "America/Indiana/Knox", "America/Indiana/Marengo", "America/Indiana/Petersburg", "America/Indiana/Tell_City",
    "America/Indiana/Vevay", "America/Indiana/Vincennes", "America/Indiana/Winamac", "America/Indianapolis",
    "America/Inuvik", "America/Iqaluit", "America/Jamaica", "America/Jujuy", "America/Juneau",
    "America/Kentucky/Louisville", "America/Kentucky/Monticello", "America/Knox_IN", "America/Kralendijk",
    "America/La_Paz", "America/Lima", "America/Los_Angeles", "America/Louisville", "America/Lower_Princes",
    "America/Maceio", "America/Managua", "America/Manaus", "America/Marigot", "America/Martinique",
    "America/Matamoros", "America/Mazatlan", "America/Mendoza", "America/Menominee", "America/Merida",
    "America/Metlakatla", "America/Mexico_City", "America/Miquelon", "America/Moncton", "America/Monterrey",
    "America/Montevideo", "America/Montreal", "America/Montserrat", "America/Nassau", "America/New_York",
    "America/Nipigon", "America/Nome", "America/Noronha", "America/North_Dakota/Beulah",
    "America/North_Dakota/Center", "America/North_Dakota/New_Salem", "America/Nuuk", "America/Ojinaga",
    "America/Panama", "America/Pangnirtung", "America/Paramaribo", "America/Phoenix", "America/Port-au-Prince",
    "America/Port_of_Spain", "America/Porto_Acre", "America/Porto_Velho", "America/Puerto_Rico",
    "America/Punta_Arenas", "America/Rainy_River", "America/Rankin_Inlet", "America/Recife", "America/Regina",
    "America/Resolute", "America/Rio_Branco", "America/Rosario", "America/Santa_Isabel", "America/Santarem",
    "America/Santiago", "America/Santo_Domingo", "America/Sao_Paulo", "America/Scoresbysund", "America/Shiprock",
    "America/Sitka", "America/St_Barthelemy", "America/St_Johns", "America/St_Kitts", "America/St_Lucia",
    "America/St_Thomas", "America/St_Vincent", "America/Swift_Current", "America/Tegucigalpa", "America/Thule",
    "America/Thunder_Bay", "America/Tijuana", "America/Toronto", "America/Tortola", "America/Vancouver",
    "America/Virgin", "America/Whitehorse", "America/Winnipeg", "America/Yakutat", "America/Yellowknife",
    "Antarctica/Casey", "Antarctica/Davis", "Antarctica/DumontDUrville", "Antarctica/Macquarie", "Antarctica/Mawson",
    "Antarctica/McMurdo", "Antarctica/Palmer", "Antarctica/Rothera", "Antarctica/South_Pole", "Antarctica/Syowa",
    "Antarctica/Vostok", "Arctic/Longyearbyen", "Asia/Aden", "Asia/Almaty", "Asia/Amman", "Asia/Anadyr",
    "Asia/Aqtau", "Asia/Aqtobe", "Asia/Ashgabat", "Asia/Ashkhabad", "Asia/Atyrau", "Asia/Baghdad", "Asia/Bahrain",
    "Asia/Baku", "Asia/Bangkok", "Asia/Barnaul", "Asia/Beirut", "Asia/Bishkek", "Asia/Brunei", "Asia/Calcutta",
    "Asia/Chita", "Asia/Choibalsan", "Asia/Chongqing", "Asia/Chungking", "Asia/Colombo", "Asia/Dacca",
    "Asia/Damascus", "Asia/Dhaka", "Asia/Dili", "Asia/Dubai", "Asia/Dushanbe", "Asia/Famagusta", "Asia/Gaza",
    "Asia/Harbin", "Asia/Hebron", "Asia/Ho_Chi_Minh", "Asia/Hong_Kong", "Asia/Hovd", "Asia/Irkutsk", "Asia/Istanbul",
    "Asia/Jakarta", "Asia/Jayapura", "Asia/Jerusalem", "Asia/Kabul", "Asia/Kamchatka", "Asia/Karachi",
    "Asia/Kashgar", "Asia/Kathmandu", "Asia/Katmandu", "Asia/Khandyga", "Asia/Kolkata", "Asia/Krasnoyarsk",
    "Asia/Kuala_Lumpur", "Asia/Kuching", "Asia/Kuwait", "Asia/Macao", "Asia/Macau", "Asia/Magadan", "Asia/Makassar",
    "Asia/Manila", "Asia/Muscat", "Asia/Nicosia", "Asia/Novokuznetsk", "Asia/Novosibirsk", "Asia/Omsk", "Asia/Oral",
    "Asia/Phnom_Penh", "Asia/Pontianak", "Asia/Pyongyang", "Asia/Qatar", "Asia/Qostanay", "Asia/Qyzylorda",
    "Asia/Rangoon", "Asia/Riyadh", "Asia/Saigon", "Asia/Sakhalin", "Asia/Samarkand", "Asia/Seoul", "Asia/Shanghai",
    "Asia/Singapore", "Asia/Srednekolymsk", "Asia/Taipei", "Asia/Tashkent", "Asia/Tbilisi", "Asia/Tehran",
    "Asia/Tel_Aviv", "Asia/Thimbu", "Asia/Thimphu", "Asia/Tokyo", "Asia/Tomsk", "Asia/Ujung_Pandang",
    "Asia/Ulaanbaatar", "Asia/Ulan_Bator", "Asia/Urumqi", "Asia/Ust-Nera", "Asia/Vientiane", "Asia/Vladivostok",
    "Asia/Yakutsk", "Asia/Yangon", "Asia/Yekaterinburg", "Asia/Yerevan", "Atlantic/Azores", "Atlantic/Bermuda",
    "Atlantic/Canary", "Atlantic/Cape_Verde", "Atlantic/Faeroe", "Atlantic/Faroe", "Atlantic/Jan_Mayen",
    "Atlantic/Madeira", "Atlantic/Reykjavik", "Atlantic/South_Georgia", "Atlantic/St_Helena", "Atlantic/Stanley",
    "Australia/ACT", "Australia/Adelaide", "Australia/Brisbane", "Australia/Broken_Hill", "Australia/Canberra",
    "Australia/Currie", "Australia/Darwin", "Australia/Eucla", "Australia/Hobart", "Australia/LHI",
    "Australia/Lindeman", "Australia/Lord_Howe", "Australia/Melbourne", "Australia/NSW", "Australia/North",
    "Australia/Perth", "Australia/Queensland", "Australia/South", "Australia/Sydney", "Australia/Tasmania",
    "Australia/Victoria", "Australia/West", "Australia/Yancowinna", "Brazil/Acre", "Brazil/DeNoronha", "Brazil/East",
    "Brazil/West", "CET", "Canada/Atlantic", "Canada/Central", "Canada/Eastern", "Canada/Mountain",
    "Canada/Newfoundland", "Canada/Pacific", "Canada/Saskatchewan", "Canada/Yukon", "Chile/Continental",
    "Chile/EasterIsland", "Cuba", "EET", "EST", "Egypt", "Eire", "Etc/GMT", "Etc/GMT+0", "Etc/GMT+1", "Etc/GMT+10",
    "Etc/GMT+11", "Etc/GMT+12", "Etc/GMT+2", "Etc/GMT+3", "Etc/GMT+4", "Etc/GMT+5", "Etc/GMT+6", "Etc/GMT+7",
    "Etc/GMT+8", "Etc/GMT+9", "Etc/GMT-0", "Etc/GMT-1", "Etc/GMT-10", "Etc/GMT-11", "Etc/GMT-12", "Etc/GMT-13",
    "Etc/GMT-14", "Etc/GMT-2", "Etc/GMT-3", "Etc/GMT-4", "Etc/GMT-5", "Etc/GMT-6", "Etc/GMT-7", "Etc/GMT-8",
    "Etc/GMT-9", "Etc/GMT0", "Etc/Greenwich", "Etc/UCT", "Etc/UTC", "Etc/Universal", "Etc/Zulu", "Europe/Amsterdam",
    "Europe/Andorra", "Europe/Astrakhan", "Europe/Athens", "Europe/Belfast", "Europe/Belgrade", "Europe/Berlin",
    "Europe/Bratislava", "Europe/Brussels", "Europe/Bucharest", "Europe/Budapest", "Europe/Busingen",
    "Europe/Chisinau", "Europe/Copenhagen", "Europe/Dublin", "Europe/Gibraltar", "Europe/Guernsey",
    "Europe/Helsinki", "Europe/Isle_of_Man", "Europe/Istanbul", "Europe/Jersey", "Europe/Kaliningrad", "Europe/Kiev",
    "Europe/Kirov", "Europe/Kyiv", "Europe/Lisbon", "Europe/Ljubljana", "Europe/London", "Europe/Luxembourg",
    "Europe/Madrid", "Europe/Malta", "Europe/Mariehamn", "Europe/Minsk", "Europe/Monaco", "Europe/Moscow",
    "Europe/Nicosia", "Europe/Oslo", "Europe/Paris", "Europe/Podgorica", "Europe/Prague", "Europe/Riga",
    "Europe/Rome", "Europe/Samara", "Europe/San_Marino", "Europe/Sarajevo", "Europe/Saratov", "Europe/Simferopol",
    "Europe/Skopje", "Europe/Sofia", "Europe/Stockholm", "Europe/Tallinn", "Europe/Tirane", "Europe/Tiraspol",
    "Europe/Ulyanovsk", "Europe/Vaduz", "Europe/Vatican", "Europe/Vienna", "Europe/Vilnius", "Europe/Volgograd",
    "Europe/Warsaw", "Europe/Zagreb", "Europe/Zurich", "GB", "GB-Eire", "GMT+0", "GMT-0", "GMT0", "Greenwich", "HST",
    "Hongkong", "Iceland", "Indian/Antananarivo", "Indian/Chagos", "Indian/Christmas", "Indian/Cocos",
    "Indian/Comoro", "Indian/Kerguelen", "Indian/Mahe", "Indian/Maldives", "Indian/Mauritius", "Indian/Mayotte",
    "Indian/Reunion", "Iran", "Israel", "Jamaica", "Japan", "Kwajalein", "Libya", "MET", "MST", "Mexico/BajaNorte",
    "Mexico/BajaSur", "Mexico/General", "NZ", "NZ-CHAT", "Navajo", "PRC", "Pacific/Apia", "Pacific/Auckland",
    "Pacific/Bougainville", "Pacific/Chatham", "Pacific/Chuuk", "Pacific/Easter", "Pacific/Efate",
    "Pacific/Enderbury", "Pacific/Fakaofo", "Pacific/Fiji", "Pacific/Funafuti", "Pacific/Galapagos",
    "Pacific/Gambier", "Pacific/Guadalcanal", "Pacific/Guam", "Pacific/Honolulu", "Pacific/Johnston",
    "Pacific/Kanton", "Pacific/Kiritimati", "Pacific/Kosrae", "Pacific/Kwajalein", "Pacific/Majuro",
    "Pacific/Marquesas", "Pacific/Midway", "Pacific/Nauru", "Pacific/Niue", "Pacific/Norfolk", "Pacific/Noumea",
    "Pacific/Pago_Pago", "Pacific/Palau", "Pacific/Pitcairn", "Pacific/Pohnpei", "Pacific/Ponape",
    "Pacific/Port_Moresby", "Pacific/Rarotonga", "Pacific/Saipan", "Pacific/Samoa", "Pacific/Tahiti",
    "Pacific/Tarawa", "Pacific/Tongatapu", "Pacific/Truk", "Pacific/Wake", "Pacific/Wallis", "Pacific/Yap", "Poland",
    "Portugal", "ROC", "ROK", "Singapore", "Turkey", "UCT", "US/Alaska", "US/Aleutian", "US/Arizona", "US/Central",
    "US/Eastern", "US/Hawaii", "US/Indiana-Starke", "US/Michigan", "US/Mountain", "US/Pacific", "US/Samoa", "UTC",
    "Universal", "W-SU", "WET", "Zulu",
];

// ----- importlib.metadata.version: the versions of the distributions the program names, read from the build
// machine's python3 when it is built (DISTS, set in main) -----
pub static DISTS: std::sync::OnceLock<&'static [(&'static str, &'static str)]> = std::sync::OnceLock::new();
// importlib.metadata's Prepared.normalize: runs of -_. become one _, lower case
pub fn dist_norm(n: &str) -> String {
    let mut o = String::new();
    let mut sep = false;
    for c in n.chars() {
        if matches!(c, '-' | '_' | '.') {
            sep = true;
            continue;
        }
        if sep {
            o.push('_');
        }
        sep = false;
        o.extend(c.to_lowercase());
    }
    if sep {
        o.push('_');
    }
    o
}
pub fn metadata_fn(n: &str, a: Args) -> R {
    match n {
        "version" => {
            let name = match a.p.first() {
                Some(V::Str(x)) if a.p.len() == 1 && a.k.is_empty() => x.text().to_string(),
                _ => return type_err("version() takes one str argument, the distribution name".into()),
            };
            let k = dist_norm(&name);
            match DISTS.get().and_then(|d| d.iter().find(|(n, _)| *n == k)) {
                Some((_, v)) => Ok(st(v.to_string())),
                None => Err(exc("PackageNotFoundError", format!("No package metadata was found for {name}"))),
            }
        }
        _ => Err(unsupported(&format!("importlib.metadata.{n}"))),
    }
}
