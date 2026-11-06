// ---------- numpy printing ----------
// str() and repr() of numpy arrays, as numpy 2's arrayprint.py makes them: Dragon4 digits (shortest that
// round-trips, capped at `precision` and then correctly rounded), one format for the whole array, wrapping at
// `linewidth`, and `...` summaries past `threshold` elements. The pieces follow FloatingFormat, IntegerFormat,
// BoolFormat, _formatArray and _extendLine in numpy/_core/arrayprint.py.

#[derive(Clone)]
pub struct NpOpts {
    pub precision: usize,
    pub suppress: bool,
    pub linewidth: usize,
    pub threshold: usize,
    pub edgeitems: usize,
}
thread_local! {
    pub static NPOPT: RefCell<NpOpts> = RefCell::new(NpOpts { precision: 8, suppress: false, linewidth: 75, threshold: 1000, edgeitems: 3 });
}
pub fn np_opts() -> NpOpts {
    NPOPT.with(|o| o.borrow().clone())
}

thread_local! {
    // Whether the value being printed is a float32 (its shortest digits are float32's).
    static NP_F32: std::cell::Cell<bool> = std::cell::Cell::new(false);
}
// f with the digit functions below working on float32 values.
pub fn np_as_f32<T>(on: bool, f: impl FnOnce() -> T) -> T {
    let was = NP_F32.with(|c| c.replace(on));
    let r = f();
    NP_F32.with(|c| c.set(was));
    r
}

// The shortest digits that round-trip and the decimal exponent: x = d[0].d[1..] * 10^e (x >= 0).
// Like numpy's Dragon4, a tie between two shortest candidates goes to the one from rounding the exact value
// half-to-even (Rust's shortest form rounds such ties up).
fn np_short(a: f64) -> (Vec<u8>, i32) {
    let f32m = NP_F32.with(|c| c.get());
    let s = if f32m { format!("{:e}", a as f32) } else { format!("{:e}", a) };
    let (m, e) = s.split_once('e').unwrap();
    let d: Vec<u8> = m.bytes().filter(|c| *c != b'.').collect();
    if a.is_finite() && a != 0.0 && d.len() >= if f32m { 6 } else { 15 } {
        let x = format!("{:.*e}", d.len() - 1, a);
        let back = if f32m { x.parse::<f32>().map(|v| v as f64 == a).unwrap_or(false) } else { x.parse::<f64>().map(|v| v == a).unwrap_or(false) };
        if back && x != s {
            let (m2, e2) = x.split_once('e').unwrap();
            return (m2.bytes().filter(|c| *c != b'.').collect(), e2.parse().unwrap());
        }
    }
    (d, e.parse().unwrap())
}

// Positional digits of |x|: (integer part, fractional digits), the shortest unique ones when they have at most
// `p` fractional digits, otherwise x correctly rounded to `p` digits (as Dragon4 in unique mode with a cutoff).
pub fn np_pos_digits(a: f64, p: usize) -> (String, String) {
    let (d, e) = np_short(a);
    let nfrac = (d.len() as i32 - 1 - e).max(0) as usize;
    if nfrac <= p {
        let ds: String = d.iter().map(|c| *c as char).collect();
        if e >= 0 {
            let e = e as usize;
            if d.len() > e + 1 {
                (ds[..e + 1].to_string(), ds[e + 1..].to_string())
            } else {
                (format!("{ds}{}", "0".repeat(e + 1 - d.len())), String::new())
            }
        } else if a == 0.0 {
            ("0".into(), String::new())
        } else {
            ("0".into(), format!("{}{ds}", "0".repeat((-e - 1) as usize)))
        }
    } else {
        let s = format!("{:.*}", p, a);
        match s.split_once('.') {
            Some((i, f)) => (i.to_string(), f.to_string()),
            None => (s, String::new()),
        }
    }
}

// Scientific digits of |x|: (first digit, fractional digits, exponent).
pub fn np_sci_digits(a: f64, p: usize) -> (String, String, i32) {
    let (d, e) = np_short(a);
    if d.len() - 1 <= p {
        let ds: String = d.iter().map(|c| *c as char).collect();
        return (ds[..1].to_string(), ds[1..].to_string(), e);
    }
    let s = format!("{:.*e}", p, a);
    let (m, ex) = s.split_once('e').unwrap();
    let (i, f) = m.split_once('.').unwrap_or((m, ""));
    (i.to_string(), f.to_string(), ex.parse().unwrap())
}

// numpy's format_float_positional(x, precision, unique=True, fractional=True, trim, sign=False, pad_left,
// pad_right, min_digits) for a finite x. trim: '.' drops trailing zeros (keeps the point), 'k' keeps them.
pub fn np_fmt_pos(x: f64, p: usize, trim: char, min_digits: usize, pad_left: usize, pad_right: usize) -> String {
    let (mut i, mut f) = np_pos_digits(x.abs(), p);
    if trim == '.' {
        while f.ends_with('0') {
            f.pop();
        }
    }
    if f.len() < min_digits && x != 0.0 {
        // Dragon4 pads to min_digits with the value's exact digits, not zeros (float32 0.7 -> 0.69999999).
        let s = format!("{:.*}", min_digits, x.abs());
        if let Some((i2, f2)) = s.split_once('.') {
            i = i2.to_string();
            f = f2.to_string();
        }
    }
    while f.len() < min_digits {
        f.push('0');
    }
    if x.is_sign_negative() {
        i.insert(0, '-');
    }
    let mut s = String::new();
    for _ in i.len()..pad_left {
        s.push(' ');
    }
    s.push_str(&i);
    s.push('.');
    s.push_str(&f);
    for _ in f.len()..pad_right {
        s.push(' ');
    }
    s
}

pub fn np_fmt_sci(x: f64, p: usize, trim: char, min_digits: usize, pad_left: usize, exp_digits: usize) -> String {
    let (mut i, mut f, mut e) = np_sci_digits(x.abs(), p);
    if trim == '.' {
        while f.ends_with('0') {
            f.pop();
        }
    }
    if f.len() < min_digits && x != 0.0 {
        let s = format!("{:.*e}", min_digits, x.abs());
        let (m, ex) = s.split_once('e').unwrap();
        let (i2, f2) = m.split_once('.').unwrap_or((m, ""));
        i = i2.to_string();
        f = f2.to_string();
        e = ex.parse().unwrap();
    }
    while f.len() < min_digits {
        f.push('0');
    }
    if x.is_sign_negative() {
        i.insert(0, '-');
    }
    let mut s = String::new();
    for _ in i.len()..pad_left {
        s.push(' ');
    }
    let ed = format!("{}", e.unsigned_abs());
    write!(s, "{i}.{f}e{}{}{ed}", if e < 0 { '-' } else { '+' }, "0".repeat(exp_digits.saturating_sub(ed.len()))).unwrap();
    s
}

// FloatingFormat (floatmode 'maxprec', sign '-') fitted to the values shown.
pub struct FFmt {
    exp: bool,
    precision: usize,
    min_digits: usize,
    trim: char,
    pad_left: usize,
    pad_right: usize,
    exp_size: usize,
}
impl FFmt {
    pub fn new(data: &[f64], o: &NpOpts) -> FFmt {
        let finite: Vec<f64> = data.iter().cloned().filter(|x| x.is_finite()).collect();
        let nz: Vec<f64> = finite.iter().filter(|x| **x != 0.0).map(|x| x.abs()).collect();
        let mut exp = false;
        if !nz.is_empty() {
            let mx = nz.iter().cloned().fold(f64::MIN, f64::max);
            let mn = nz.iter().cloned().fold(f64::MAX, f64::min);
            // numpy: the max cutoff is 10**min(8, finfo precision), 1e6 for float32, and the tests run in the
            // array's dtype (float32 0.0001 is below 1e-4 as a double but not as a float32).
            if NP_F32.with(|c| c.get()) {
                let (mx, mn) = (mx as f32, mn as f32);
                if mx >= 1e6 || (!o.suppress && (mn < 0.0001 || mx / mn > 1000.0)) {
                    exp = true;
                }
            } else if mx >= 1e8 || (!o.suppress && (mn < 0.0001 || mx / mn > 1000.0)) {
                exp = true;
            }
        }
        let mut f = FFmt { exp, precision: o.precision, min_digits: 0, trim: '.', pad_left: 0, pad_right: 0, exp_size: 0 };
        if finite.is_empty() {
            f.exp = false;
        } else if exp {
            let mut pl = 0;
            let mut fr = 0;
            let mut es = 0;
            for x in &finite {
                let (mut i, mut fd, e) = np_sci_digits(x.abs(), o.precision);
                while fd.ends_with('0') {
                    fd.pop();
                }
                if x.is_sign_negative() {
                    i.insert(0, '-');
                }
                pl = pl.max(i.len());
                fr = fr.max(fd.len());
                es = es.max(format!("{}", e.unsigned_abs()).len().max(2));
            }
            f.exp_size = es;
            f.trim = 'k';
            f.precision = fr;
            f.min_digits = fr;
            f.pad_left = pl;
            f.pad_right = es + 2 + fr;
        } else {
            let mut pl = 0;
            let mut pr = 0;
            for x in &finite {
                let (mut i, mut fd) = np_pos_digits(x.abs(), o.precision);
                while fd.ends_with('0') {
                    fd.pop();
                }
                if x.is_sign_negative() {
                    i.insert(0, '-');
                }
                pl = pl.max(i.len());
                pr = pr.max(fd.len());
            }
            f.pad_left = pl;
            f.pad_right = pr;
        }
        if finite.len() != data.len() {
            let neginf = data.iter().any(|x| x.is_infinite() && *x < 0.0) as usize;
            let off = f.pad_right + 1;
            f.pad_left = f.pad_left.max((3usize).saturating_sub(off)).max((3 + neginf).saturating_sub(off));
        }
        f
    }
    pub fn fmt(&self, x: f64) -> String {
        if !x.is_finite() {
            let r = if x.is_nan() {
                "nan"
            } else if x < 0.0 {
                "-inf"
            } else {
                "inf"
            };
            let w = self.pad_left + self.pad_right + 1;
            return format!("{}{r}", " ".repeat(w.saturating_sub(r.len())));
        }
        if self.exp {
            np_fmt_sci(x, self.precision, self.trim, self.min_digits, self.pad_left, self.exp_size)
        } else {
            np_fmt_pos(x, self.precision, self.trim, self.min_digits, self.pad_left, self.pad_right)
        }
    }
}

enum EFmt {
    F(FFmt, bool),
    I(usize),
    B,
    U,
}
impl EFmt {
    fn new(data: &Buf, o: &NpOpts) -> EFmt {
        match data {
            Buf::F(v) => EFmt::F(FFmt::new(v, o), false),
            Buf::F32(v) => EFmt::F(np_as_f32(true, || FFmt::new(&v.iter().map(|x| *x as f64).collect::<Vec<_>>(), o)), true),
            Buf::I(v) => EFmt::I(v.iter().map(|x| x.to_string().len()).max().unwrap_or(0)),
            Buf::I32(v) => EFmt::I(v.iter().map(|x| x.to_string().len()).max().unwrap_or(0)),
            Buf::B(_) => EFmt::B,
            Buf::U(..) => EFmt::U,
        }
    }
    fn fmt(&self, x: Sc, zero_d: bool) -> String {
        match (self, x) {
            (EFmt::F(f, w), x) => np_as_f32(*w, || f.fmt(x.f())),
            (EFmt::I(w), x) => format!("{:>w$}", x.i(), w = *w),
            // a string shows as Python's repr of it, unpadded
            (EFmt::U, x) => str_repr(&np_num_str(x)),
            (EFmt::B, x) => (if x.b() {
                if zero_d {
                    "True"
                } else {
                    " True"
                }
            } else {
                "False"
            })
            .to_string(),
        }
    }
}

fn np_extend_line(s: &mut String, line: &mut String, word: &str, width: usize, prefix: &str) {
    let mut wrap = line.chars().count() + word.chars().count() > width;
    if line.chars().count() <= prefix.chars().count() {
        wrap = false;
    }
    if wrap {
        s.push_str(line.trim_end());
        s.push('\n');
        *line = prefix.to_string();
    }
    line.push_str(word);
}

struct FmtCtx<'a> {
    vals: &'a Buf,
    shape: &'a [usize],
    cst: Vec<usize>,
    f: EFmt,
    sep: &'a str,
    edge: usize,
    summary: bool,
}
impl<'a> FmtCtx<'a> {
    fn rec(&self, index: &mut Vec<usize>, hang: &str, width: usize) -> String {
        let axis = index.len();
        let left = self.shape.len() - axis;
        if left == 0 {
            let p: usize = index.iter().zip(&self.cst).map(|(i, s)| i * s).sum();
            return self.f.fmt(self.vals.get(p), self.shape.is_empty());
        }
        let next_hang = format!("{hang} ");
        let next_width = width - 1;
        let n = self.shape[axis];
        let show = self.summary && 2 * self.edge < n;
        let (lead, trail) = if show { (self.edge, self.edge) } else { (0, n) };
        let mut s = String::new();
        if left == 1 {
            let elem_width = width - self.sep.trim_end().len().max(1);
            let mut line = hang.to_string();
            for i in 0..lead {
                index.push(i);
                let w = self.rec(index, &next_hang, next_width);
                index.pop();
                np_extend_line(&mut s, &mut line, &w, elem_width, hang);
                line.push_str(self.sep);
            }
            if show {
                np_extend_line(&mut s, &mut line, "...", elem_width, hang);
                line.push_str(self.sep);
            }
            for i in (2..=trail).rev() {
                index.push(n - i);
                let w = self.rec(index, &next_hang, next_width);
                index.pop();
                np_extend_line(&mut s, &mut line, &w, elem_width, hang);
                line.push_str(self.sep);
            }
            index.push(n - 1);
            let w = self.rec(index, &next_hang, next_width);
            index.pop();
            np_extend_line(&mut s, &mut line, &w, elem_width, hang);
            s.push_str(&line);
        } else {
            let line_sep = format!("{}{}", self.sep.trim_end(), "\n".repeat(left - 1));
            for i in 0..lead {
                index.push(i);
                let w = self.rec(index, &next_hang, next_width);
                index.pop();
                write!(s, "{hang}{w}{line_sep}").unwrap();
            }
            if show {
                write!(s, "{hang}...{line_sep}").unwrap();
            }
            for i in (2..=trail).rev() {
                index.push(n - i);
                let w = self.rec(index, &next_hang, next_width);
                index.pop();
                write!(s, "{hang}{w}{line_sep}").unwrap();
            }
            index.push(n - 1);
            let w = self.rec(index, &next_hang, next_width);
            index.pop();
            write!(s, "{hang}{w}").unwrap();
        }
        format!("[{}]", &s[hang.len()..])
    }
    // The values the summary shows (what numpy fits the format to).
    fn shown(&self, index: &mut Vec<usize>, out: &mut Vec<usize>) {
        let axis = index.len();
        if axis == self.shape.len() {
            out.push(index.iter().zip(&self.cst).map(|(i, s)| i * s).sum());
            return;
        }
        let n = self.shape[axis];
        let show = self.summary && 2 * self.edge < n;
        let idx: Vec<usize> = if show { (0..self.edge).chain(n - self.edge..n).collect() } else { (0..n).collect() };
        for i in idx {
            index.push(i);
            self.shown(index, out);
            index.pop();
        }
    }
}

// array2string(a, separator=sep, prefix=prefix, suffix=suffix) for values in C order.
pub fn np_array2string(vals: &Buf, shape: &[usize], sep: &str, prefix: &str, suffix: &str) -> String {
    let o = np_opts();
    let size: usize = shape.iter().product();
    if size == 0 {
        return "[]".into();
    }
    let summary = size > o.threshold;
    let mut cst = vec![1usize; shape.len()];
    for k in (0..shape.len().saturating_sub(1)).rev() {
        cst[k] = cst[k + 1] * shape[k + 1];
    }
    let mut ctx = FmtCtx { vals, shape, cst, f: EFmt::B, sep, edge: o.edgeitems, summary };
    let mut ps = vec![];
    ctx.shown(&mut vec![], &mut ps);
    ctx.f = EFmt::new(&vals.pick(&ps), &o);
    let hang = format!(" {}", " ".repeat(prefix.chars().count()));
    ctx.rec(&mut vec![], &hang, o.linewidth - suffix.len())
}

pub fn np_array_str(vals: &Buf, shape: &[usize]) -> String {
    if shape.is_empty() {
        return np_sc_str(vals.get(0));
    }
    np_array2string(vals, shape, " ", "", "")
}

pub fn np_shape_repr(shape: &[usize]) -> String {
    match shape.len() {
        1 => format!("({},)", shape[0]),
        _ => format!("({})", shape.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")),
    }
}

pub fn np_array_repr(vals: &Buf, shape: &[usize]) -> String {
    let size: usize = shape.iter().product();
    // numpy 2: a summarized array (more items than the threshold) also shows its shape.
    let big = size > np_opts().threshold;
    let skip = size > 0 && vals.dt().default_dt() && !big;
    let suffix = if skip { ")" } else { "," };
    let lst = if size > 0 || shape == [0] { np_array2string(vals, shape, ", ", "array(", suffix) } else { format!("[], shape={}", np_shape_repr(shape)) };
    let s = format!("array({lst}{suffix}");
    if skip {
        return s;
    }
    let mut extras = vec![];
    if big {
        extras.push(format!("shape={}", np_shape_repr(shape)));
    }
    if !(size > 0 && vals.dt().default_dt()) {
        extras.push(format!("dtype={}", match vals.dt() {
            NT::U(_) => format!("'{}'", vals.dt().full_name()),
            d => d.name().to_string(),
        }));
    }
    let dt = format!("{})", extras.join(", "));
    let last = s[s.rfind('\n').map_or(0, |i| i + 1)..].chars().count();
    if last + dt.len() + 1 > np_opts().linewidth {
        format!("{s}\n      {dt}")
    } else {
        format!("{s} {dt}")
    }
}

// str() of a float32 scalar (floattype_str_either: positional from 1e-4 up to 1e6, else scientific; shortest
// float32 digits).
pub fn np_f32_str(x: f32) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return (if x < 0.0 { "-inf" } else { "inf" }).into();
    }
    let a = (x as f64).abs();
    np_as_f32(true, || {
        if x == 0.0 || (a < 1e6 && a >= 1e-4) {
            // trim LeaveOneZero: 1.0, 0.5
            let mut s = np_fmt_pos(x as f64, usize::MAX >> 1, '.', 0, 0, 0);
            if s.ends_with('.') {
                s.push('0');
            }
            s
        } else {
            // trim DptZeros: 1e+06, 1.5e-05
            let mut s = np_fmt_sci(x as f64, usize::MAX >> 1, '.', 0, 0, 2);
            if let Some(i) = s.find(".e") {
                s.replace_range(i..i + 1, "");
            }
            s
        }
    })
}
pub fn np_sc_str(x: Sc) -> String {
    match x {
        Sc::F(f) => float_repr(f),
        Sc::I(i) => i.to_string(),
        Sc::B(b) => (if b { "True" } else { "False" }).into(),
        Sc::I32(i) => i.to_string(),
        Sc::F32(f) => np_f32_str(f),
        Sc::U(i) => np_sget(i).to_string(),
    }
}
pub fn np_sc_repr(x: Sc) -> String {
    match x {
        Sc::F(f) => format!("np.float64({})", float_repr(f)),
        Sc::I(i) => format!("np.int64({i})"),
        Sc::B(b) => (if b { "np.True_" } else { "np.False_" }).into(),
        Sc::I32(i) => format!("np.int32({i})"),
        Sc::F32(f) => format!("np.float32({})", np_f32_str(f)),
        Sc::U(i) => format!("np.str_({})", str_repr(&np_sget(i))),
    }
}
