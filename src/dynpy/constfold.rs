// CPython 3.13's constant folding (Python/ast_opt.c) for the values whose identity a program can see: a str,
// bytes or tuple that CPython's compiler folds is one constant, shared by every equal constant of the module
// (the compiler's constant cache), so `[(1, 2), (1, 2)]` holds one tuple twice and pickle writes it once. The
// dynamic build makes those values module constants too (see Em::konst_cv).
use super::parse::{Arg, Ex, FPart};

#[derive(Clone, Debug)]
pub enum CV {
    Int(i128),
    // an int literal too big for i128: kept as its text, never computed with
    BigLit(String),
    Float(f64),
    // an imaginary literal (its imaginary part)
    Imag(f64),
    Str(String),
    // the chars (all below 256) of the str the parser made, as bi___bytes_lit reads them
    Bytes(String),
    NoneV,
    Bool(bool),
    Ellipsis,
    Tuple(Vec<CV>),
}

pub enum Fold {
    // CPython folds it to this constant
    Const(CV),
    // CPython leaves it to run time (a name, a call, an error at run time, past a size limit)
    No,
    // CPython folds it, or may, in a way this does not compute (2 ** 0.5, 1 << 300, complex arithmetic)
    Unknown,
}

// ast_opt.c's limits
const MAX_INT_SIZE: u32 = 128;
const MAX_COLLECTION_SIZE: i128 = 256;
const MAX_STR_SIZE: i128 = 4096;
const MAX_TOTAL_ITEMS: i128 = 1024;

fn bits(v: i128) -> u32 {
    128 - v.unsigned_abs().leading_zeros()
}

fn int_of(c: &CV) -> Option<i128> {
    match c {
        CV::Int(i) => Some(*i),
        CV::Bool(b) => Some(*b as i128),
        _ => None,
    }
}

fn parse_int(t: &str) -> CV {
    let s = t.replace('_', "");
    let (body, base) = if let Some(r) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (r.to_string(), 16)
    } else if let Some(r) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        (r.to_string(), 8)
    } else if let Some(r) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        (r.to_string(), 2)
    } else {
        (s.clone(), 10)
    };
    match i128::from_str_radix(&body, base) {
        Ok(v) => CV::Int(v),
        Err(_) => CV::BigLit(t.to_string()),
    }
}

// check_complexity: the items of a tuple and of the tuples in it, counted against `limit`
fn complexity(c: &CV) -> i128 {
    match c {
        CV::Tuple(xs) => xs.len() as i128 + xs.iter().map(complexity).sum::<i128>(),
        _ => 0,
    }
}

fn truthy(c: &CV) -> Option<bool> {
    Some(match c {
        CV::Int(i) => *i != 0,
        CV::BigLit(_) => true,
        CV::Float(f) => *f != 0.0,
        CV::Imag(f) => *f != 0.0,
        CV::Str(s) | CV::Bytes(s) => !s.is_empty(),
        CV::NoneV => false,
        CV::Bool(b) => *b,
        CV::Ellipsis => true,
        CV::Tuple(xs) => !xs.is_empty(),
    })
}

fn repeat(c: &CV, n: i128) -> Fold {
    match c {
        CV::Str(s) | CV::Bytes(s) => {
            let size = s.chars().count() as i128;
            if size != 0 && (n < 0 || n > MAX_STR_SIZE / size) {
                return Fold::No;
            }
            let r = s.repeat(n.max(0) as usize);
            Fold::Const(if matches!(c, CV::Str(_)) { CV::Str(r) } else { CV::Bytes(r) })
        }
        CV::Tuple(xs) => {
            let size = xs.len() as i128;
            if size != 0 && (n < 0 || n > MAX_COLLECTION_SIZE / size || (n > 0 && complexity(c) > MAX_TOTAL_ITEMS / n)) {
                return Fold::No;
            }
            let mut out = vec![];
            for _ in 0..n.max(0) {
                out.extend(xs.iter().cloned());
            }
            Fold::Const(CV::Tuple(out))
        }
        _ => Fold::Unknown,
    }
}

fn floor_div(a: i128, b: i128) -> Option<i128> {
    let q = a.checked_div(b)?;
    Some(if (a % b != 0) && ((a < 0) != (b < 0)) { q - 1 } else { q })
}

fn num_of(c: &CV) -> Option<f64> {
    match c {
        CV::Float(f) => Some(*f),
        // exact as a float below 2**53 (larger ones round, which this does not model)
        _ => int_of(c).filter(|i| i.unsigned_abs() <= 1 << 53).map(|i| i as f64),
    }
}

fn bin(op: &str, l: &CV, r: &CV) -> Fold {
    use Fold::*;
    if let (CV::Bool(a), CV::Bool(b)) = (l, r) {
        match op {
            "&" => return Const(CV::Bool(a & b)),
            "|" => return Const(CV::Bool(a | b)),
            "^" => return Const(CV::Bool(a ^ b)),
            _ => {}
        }
    }
    if let (Some(a), Some(b)) = (int_of(l), int_of(r)) {
        let v = match op {
            "+" => a.checked_add(b),
            "-" => a.checked_sub(b),
            "*" if bits(a) + bits(b) > MAX_INT_SIZE => return No,
            "*" => a.checked_mul(b),
            "//" | "%" | "/" if b == 0 => return No,
            "/" => return num_of(l).zip(num_of(r)).map_or(Unknown, |(x, y)| float_const(x / y)),
            "//" => floor_div(a, b),
            "%" => floor_div(a, b).map(|q| a - q * b),
            "&" => Some(a & b),
            "|" => Some(a | b),
            "^" => Some(a ^ b),
            ">>" if b < 0 => return No,
            ">>" => Some(if b >= 127 { if a < 0 { -1 } else { 0 } } else { a >> b }),
            "<<" if b < 0 => return No,
            "<<" if a == 0 => Some(0),
            "<<" if bits(a) > MAX_INT_SIZE || b > MAX_INT_SIZE as i128 || bits(a) as i128 + b > MAX_INT_SIZE as i128 => return No,
            "<<" => a.checked_shl(b as u32).filter(|v| v >> b == a),
            "**" if b < 0 => return Unknown,
            "**" if a != 0 && b > 0 && bits(a) as i128 > MAX_INT_SIZE as i128 / b => return No,
            "**" => a.checked_pow(b as u32),
            _ => return Unknown,
        };
        return match v {
            Some(v) => Const(CV::Int(v)),
            None => Unknown,
        };
    }
    match (op, l, r) {
        ("+", CV::Str(a), CV::Str(b)) => Const(CV::Str(format!("{a}{b}"))),
        ("+", CV::Bytes(a), CV::Bytes(b)) => Const(CV::Bytes(format!("{a}{b}"))),
        ("+", CV::Tuple(a), CV::Tuple(b)) => Const(CV::Tuple(a.iter().chain(b.iter()).cloned().collect())),
        ("*", CV::Str(_) | CV::Bytes(_) | CV::Tuple(_), n) | ("*", n, CV::Str(_) | CV::Bytes(_) | CV::Tuple(_)) if int_of(n).is_some() => {
            let n = int_of(n).unwrap();
            let seq = if int_of(l).is_some() { r } else { l };
            repeat(seq, n)
        }
        // a str or bytes on the left of % is left to run time (safe_mod)
        ("%", CV::Str(_) | CV::Bytes(_), _) => No,
        // any other mix of a str, bytes or tuple with something raises TypeError at run time (not folded);
        // numbers with floats, complex or big ints: folded by CPython, not computed here
        (_, CV::Str(_) | CV::Bytes(_) | CV::Tuple(_) | CV::NoneV | CV::Ellipsis, _) | (_, _, CV::Str(_) | CV::Bytes(_) | CV::Tuple(_) | CV::NoneV | CV::Ellipsis) => No,
        _ => match (num_of(l), num_of(r)) {
            // a float with a float or a small int: IEEE double arithmetic, as CPython's float ops
            (Some(x), Some(y)) if matches!(l, CV::Float(_)) || matches!(r, CV::Float(_)) => match op {
                "+" => float_const(x + y),
                "-" => float_const(x - y),
                "*" => float_const(x * y),
                "/" if y == 0.0 => No,
                "/" => float_const(x / y),
                _ => Unknown,
            },
            _ => Unknown,
        },
    }
}

// a float result; NaN is left unmodelled (its payload)
fn float_const(f: f64) -> Fold {
    if f.is_nan() { Fold::Unknown } else { Fold::Const(CV::Float(f)) }
}

fn unary(op: &str, x: &CV) -> Fold {
    use Fold::*;
    match (op, x) {
        ("not", x) => truthy(x).map_or(Unknown, |t| Const(CV::Bool(!t))),
        ("-", CV::Int(i)) => i.checked_neg().map_or(Unknown, |v| Const(CV::Int(v))),
        ("-", CV::Bool(b)) => Const(CV::Int(-(*b as i128))),
        ("-", CV::Float(f)) => Const(CV::Float(-f)),
        ("+", CV::Int(i)) => Const(CV::Int(*i)),
        ("+", CV::Bool(b)) => Const(CV::Int(*b as i128)),
        ("+", CV::Float(f)) => Const(CV::Float(*f)),
        ("~", CV::Int(i)) => Const(CV::Int(!i)),
        (_, CV::Str(_) | CV::Bytes(_) | CV::Tuple(_) | CV::NoneV | CV::Ellipsis) => No,
        // ~True (a DeprecationWarning), -1j, -(big literal)
        _ => Unknown,
    }
}

// What CPython's compiler makes of `e`: a constant, run-time code, or a fold this does not model.
pub fn fold(e: &Ex) -> Fold {
    use Fold::*;
    match e {
        Ex::Int(t) => Const(parse_int(t)),
        Ex::Float(f) => Const(CV::Float(*f)),
        Ex::Str(s) => Const(CV::Str(s.clone())),
        // an f-string with no fields is a str constant
        Ex::FStr(parts) if parts.iter().all(|p| matches!(p, FPart::Lit(_))) => {
            Const(CV::Str(parts.iter().map(|p| if let FPart::Lit(t) = p { t.as_str() } else { "" }).collect()))
        }
        Ex::NoneV => Const(CV::NoneV),
        Ex::True => Const(CV::Bool(true)),
        Ex::False => Const(CV::Bool(false)),
        Ex::Ellipsis => Const(CV::Ellipsis),
        Ex::Call(f, a) if matches!(&**f, Ex::Name(n) if n == "__bytes_lit") => match &a[..] {
            [Arg::Pos(Ex::Str(b))] => Const(CV::Bytes(b.clone())),
            _ => No,
        },
        Ex::Call(f, a) if matches!(&**f, Ex::Name(n) if n == "__complex_lit") => match &a[..] {
            [Arg::Pos(Ex::Float(x))] => Const(CV::Imag(*x)),
            _ => No,
        },
        Ex::Tuple(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            let mut unknown = false;
            for x in xs {
                match fold(x) {
                    Const(c) => out.push(c),
                    No => return No,
                    Unknown => unknown = true,
                }
            }
            if unknown { Unknown } else { Const(CV::Tuple(out)) }
        }
        Ex::Bin(op, l, r) => match (fold(l), fold(r)) {
            (Const(a), Const(b)) => bin(op, &a, &b),
            (No, _) | (_, No) => No,
            _ => Unknown,
        },
        Ex::Unary(op, x) => match fold(x) {
            Const(c) => unary(op, &c),
            other => other,
        },
        _ => No,
    }
}

// CPython interns a str constant made only of ASCII letters, digits and underscores (all_name_chars), so equal
// ones in different modules are one object; other constants are shared within their module only.
pub fn all_name_chars(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
