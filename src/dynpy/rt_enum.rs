
// ---------- enum: Enum, auto, unique ----------
// A class with Enum among its bases turns its plain class attributes into members when it is made
// (make_class). Members are instances with _name_/_value_ (also name/value); a value seen before makes an
// alias. The class iterates, has len, `in`, C[name] and C(value) as CPython's EnumType gives it.
thread_local! {
    static ENUM_CLS: RefCell<Option<(Rc<Class>, Rc<Class>)>> = RefCell::new(None);
    // IntEnum, StrEnum, Flag, IntFlag
    static ENUM_MORE: RefCell<Option<[Rc<Class>; 4]>> = RefCell::new(None);
}
fn enum_classes() -> (Rc<Class>, Rc<Class>) {
    ENUM_CLS.with(|e| {
        let mut e = e.borrow_mut();
        if e.is_none() {
            let en = Class::new("Enum", "enum", vec![bclass("object")], BT::Other);
            en.set("__repr__", builtin("__repr__", |a: Args| enum_text(a.p.first().unwrap_or(&V::None), true)));
            en.set("__str__", builtin("__str__", |a: Args| enum_text(a.p.first().unwrap_or(&V::None), false)));
            let au = Class::new("auto", "enum", vec![bclass("object")], BT::Other);
            *e = Some((en, au));
        }
        e.clone().unwrap()
    })
}
// IntEnum, StrEnum, Flag and IntFlag, as enum.py builds them on Enum.
fn enum_more() -> [Rc<Class>; 4] {
    ENUM_MORE.with(|e| {
        if let Some(k) = e.borrow().as_ref() {
            return k.clone();
        }
        let en = enum_base();
        // ReprEnum: str() and format() are the mixed-in type's
        let mix_str = || builtin("__str__", |a: Args| match a.p.first().and_then(inner_of) {
            Some(i) => Ok(st(to_str(&i)?)),
            None => Ok(s("")),
        });
        let ie = Class::new("IntEnum", "enum", vec![bclass("int"), en.clone()], BT::Other);
        ie.set("__str__", mix_str());
        ie.set("__bv_reprenum__", V::Bool(true));
        let se = Class::new("StrEnum", "enum", vec![bclass("str"), en.clone()], BT::Other);
        se.set("__str__", mix_str());
        se.set("__bv_reprenum__", V::Bool(true));
        let fl = Class::new("Flag", "enum", vec![en.clone()], BT::Other);
        flag_methods(&fl);
        let ifl = Class::new("IntFlag", "enum", vec![bclass("int"), fl.clone()], BT::Other);
        // Flag's operators, not int's (IntFlag sets them in its own body too)
        flag_methods(&ifl);
        ifl.set("__str__", mix_str());
        ifl.set("__bv_reprenum__", V::Bool(true));
        ifl.set("__bv_keep__", V::Bool(true));
        let k = [ie, se, fl, ifl];
        *e.borrow_mut() = Some(k.clone());
        k
    })
}
pub fn enum_class(n: &str) -> Option<Rc<Class>> {
    let [ie, se, fl, ifl] = enum_more();
    Some(match n {
        "IntEnum" => ie,
        "StrEnum" => se,
        "Flag" => fl,
        "IntFlag" => ifl,
        _ => return None,
    })
}
fn is_flag(c: &Rc<Class>) -> bool {
    ENUM_MORE.with(|e| e.borrow().as_ref().is_some_and(|k| c.is_sub(&k[2])))
}
fn is_str_enum(c: &Rc<Class>) -> bool {
    ENUM_MORE.with(|e| e.borrow().as_ref().is_some_and(|k| c.is_sub(&k[1])))
}
// IntEnum, StrEnum, IntFlag: format() is the value's, not str(member)'s.
pub fn is_repr_enum(c: &Rc<Class>) -> bool {
    c.lookup("__bv_reprenum__").is_some()
}
pub fn enum_base() -> Rc<Class> {
    enum_classes().0
}
pub fn is_enum(c: &Rc<Class>) -> bool {
    let en = enum_base();
    !Rc::ptr_eq(c, &en) && c.module != "enum" && c.is_sub(&en)
}
fn enum_text(v: &V, rep: bool) -> R {
    let V::Obj(o) = v else { return type_err("descriptor needs an Enum member".into()) };
    let name = match o.get("_name_") {
        Some(V::Str(n)) => n.s.clone(),
        _ => String::new(),
    };
    Ok(s(&if rep {
        format!("<{}.{}: {}>", o.cls.name, name, repr(&o.get("_value_").unwrap_or(V::None))?)
    } else {
        format!("{}.{}", o.cls.name, name)
    }))
}
pub fn enum_members(c: &Rc<Class>) -> V {
    c.own("__bv_members__").unwrap_or_else(|| list(vec![]))
}
// Names in an Enum body that CPython does not make members: _sunder_ and __dunder__ names, and private
// names (_Cls__x, as the class body sees `__x`). Any other name starting with `_` is a member.
fn enum_not_member(cls: &str, n: &str) -> bool {
    let b = n.as_bytes();
    let sunder = b.len() > 2 && b[0] == b'_' && b[1] != b'_' && b[b.len() - 1] == b'_' && b[b.len() - 2] != b'_';
    let dunder = b.len() > 4 && n.starts_with("__") && n.ends_with("__") && b[2] != b'_' && b[b.len() - 3] != b'_';
    let private = n.strip_prefix('_').and_then(|r| r.strip_prefix(cls.trim_start_matches('_'))).map_or(false, |r| r.starts_with("__"));
    sunder || dunder || private || n.starts_with("__")
}
// What the class being defined will be, from its bases (before make_class builds it): bit 1 Enum, bit 2 a
// Flag subclass, bit 4 a StrEnum subclass. The class body uses this to give each auto() its value as soon as
// the member is assigned, as CPython's _EnumDict does.
pub fn enum_auto_ctx(bases: &[&V]) -> i64 {
    let en = enum_base();
    let mut ctx = 0;
    for b in bases {
        if let V::Class(c) = b {
            if c.is_sub(&en) {
                ctx |= 1;
            }
        }
    }
    if ctx == 0 {
        return 0;
    }
    let k = enum_more();
    for b in bases {
        if let V::Class(c) = b {
            if c.is_sub(&k[2]) {
                ctx |= 2;
            }
            if c.is_sub(&k[1]) {
                ctx |= 4;
            }
        }
    }
    ctx
}
// The value auto() gets in an Enum class body: 1, then the next value for Enum; the next power of two for
// Flag; the lower-cased member name for StrEnum. `last` holds the values assigned so far, in order.
fn auto_next(ctx: i64, name: &str, last: &[V]) -> R {
    if ctx & 2 != 0 {
        let mut best: Option<i64> = None;
        for x in last {
            match x {
                V::Int(i) => best = Some(best.map_or(*i, |b| b.max(*i))),
                V::Bool(b) => best = Some(best.map_or(*b as i64, |m| m.max(*b as i64))),
                x => return type_err(format!("invalid flag value {}", repr(x)?)),
            }
        }
        let b = best.unwrap_or(0);
        let hb = 64 - (b.max(0) as u64).leading_zeros() as i64 - 1;
        if hb >= 61 {
            return Err(unsupported("Flag values past 2**62"));
        }
        return Ok(V::Int(1i64 << (hb + 1)));
    }
    if ctx & 4 != 0 {
        return Ok(st(name.to_lowercase()));
    }
    let mut best: Option<V> = None;
    for x in last {
        match (&best, x) {
            (_, V::Int(_) | V::Big(_) | V::Bool(_)) => {
                if best.as_ref().map_or(Ok(true), |b| lt(b, x))? {
                    best = Some(x.clone());
                }
            }
            _ => return Err(unsupported("enum.auto() after a value that is not an int")),
        }
    }
    match best {
        Some(b) => add(&b, &V::Int(1)),
        None => Ok(V::Int(1)),
    }
}
// Every name store in the body of an Enum class goes through here: the members' values are kept in order,
// and an auto() gets (and memoises, as CPython does) the value for its position.
pub fn enum_auto_store(last: &mut Vec<V>, ctx: i64, name: &str, v: V) -> R {
    if ctx == 0 || (name.starts_with('_') && enum_not_member("", name)) || matches!(&v, V::Unbound | V::Func(_) | V::Class(_) | V::Prop(_) | V::Bound(_) | V::BMeth(_)) {
        return Ok(v);
    }
    let au = enum_classes().1;
    match &v {
        V::Obj(o) if Rc::ptr_eq(&o.cls, &au) => {
            if let Some(x) = o.get("value") {
                // the same auto() assigned again: CPython reuses the value it was given first
                last.push(x.clone());
                return Ok(x);
            }
            let x = auto_next(ctx, name, last)?;
            o.set("value", x.clone());
            last.push(x.clone());
            Ok(x)
        }
        _ => {
            last.push(v.clone());
            Ok(v)
        }
    }
}
// Called by make_class for a subclass of Enum.
pub fn enum_setup(c: &Rc<Class>) -> Result<(), E> {
    let au = enum_classes().1;
    if c.mro.borrow().iter().skip(1).any(|b| is_enum(b) && matches!(b.own("__bv_members__"), Some(V::List(l)) if !l.borrow().is_empty())) {
        return type_err(format!("<enum '{}'> cannot extend an enum with members", c.name));
    }
    let attrs = c.attrs.borrow().clone();
    let init = c.lookup("__init__").filter(|f| !is_builtin_init(f));
    let mut members: Vec<V> = vec![];
    let mut values: Vec<V> = vec![];
    let mut v2m: Vec<(V, V)> = vec![];
    let map = dict_new();
    for (n, v) in attrs {
        if (n.starts_with('_') && enum_not_member(c.name, n)) || matches!(v, V::Func(_) | V::Class(_) | V::Prop(_) | V::Bound(_) | V::BMeth(_)) {
            continue;
        }
        let val = match &v {
            V::Obj(o) if Rc::ptr_eq(&o.cls, &au) && is_str_enum(c) => auto_next(4, n, &values)?,
            V::Obj(o) if Rc::ptr_eq(&o.cls, &au) && is_flag(c) => auto_next(2, n, &values)?,
            V::Obj(o) if Rc::ptr_eq(&o.cls, &au) => auto_next(1, n, &values)?,
            _ => v.clone(),
        };
        // StrEnum values are str; IntEnum and IntFlag values are int(value)
        let val = if is_str_enum(c) {
            match &val {
                V::Str(_) => val,
                x => return type_err(format!("{} is not a string", repr(x)?)),
            }
        } else if matches!(c.base_bt(), BT::Int) && !matches!(val, V::Int(_) | V::Big(_)) {
            bi_int(Args::p(vec![val]))?
        } else {
            val
        };
        if is_flag(c) && matches!(val, V::Big(_)) {
            return Err(unsupported("Flag values past 2**62"));
        }
        values.push(val.clone());
        let mut found = None;
        for m in &members {
            if let V::Obj(o) = m {
                let mv = o.get("_value_").unwrap_or(V::None);
                if is(&mv, &val) || eq(&mv, &val)? {
                    found = Some(m.clone());
                    break;
                }
            }
        }
        let m = match found {
            Some(m) => m,
            None => {
                let o = Obj::new(c.clone());
                o.set("_name_", s(n));
                o.set("_value_", val.clone());
                o.set("name", s(n));
                o.set("value", val.clone());
                // class C(str, Enum) / (int, Enum): the member is also the str or int
                if !matches!(c.base_bt(), BT::Object | BT::Other) {
                    o.set_inner(Some(val.clone()));
                }
                let mv = V::Obj(o);
                if let Some(f) = &init {
                    let mut p = vec![mv.clone()];
                    match &val {
                        V::Tuple(t) => p.extend(t.iter().cloned()),
                        _ => p.push(val.clone()),
                    }
                    call(f, Args::p(p))?;
                }
                let alias = is_flag(c) && matches!(val, V::Int(i) if i.count_ones() != 1);
                if !alias {
                    members.push(mv.clone());
                }
                v2m.push((val.clone(), mv.clone()));
                mv
            }
        };
        c.set(n, m.clone());
        setitem(&map, &s(n), m.clone())?;
    }
    if is_flag(c) {
        let (mut fm, mut sm) = (0i64, 0i64);
        for (v, _) in &v2m {
            if let V::Int(i) = v {
                fm |= i;
                if i.count_ones() == 1 {
                    sm |= i;
                }
            }
        }
        let all = if fm <= 0 { 0 } else { ((1i128 << (64 - (fm as u64).leading_zeros())) - 1) as i64 };
        c.set("_flag_mask_", V::Int(fm));
        c.set("_singles_mask_", V::Int(sm));
        c.set("_all_bits_", V::Int(all));
        let vals: Vec<i64> = members.iter().filter_map(|m| if let V::Obj(o) = m { if let Some(V::Int(i)) = o.get("_value_") { Some(i) } else { None } } else { None }).collect();
        let mut sorted = vals.clone();
        sorted.sort();
        c.set("__bv_bydef__", V::Bool(sorted != vals));
        let d = dict_new();
        for (v, m) in &v2m {
            if let V::Dict(dd) = &d {
                if dd.borrow().get(v)?.is_none() {
                    dd.borrow_mut().set(v.clone(), m.clone())?;
                }
            }
        }
        c.set("_value2member_map_", d);
    }
    c.set("__bv_members__", list(members));
    c.set("__members__", map.clone());
    c.set("_member_map_", map);
    Ok(())
}

// ---------- Flag ----------
// Flag operators on a member whose value is not an int: CPython's errors are not modelled.
fn int_flag_value(v: &V) -> Result<(), E> {
    match v {
        V::Obj(o) if !matches!(o.get("_value_"), Some(V::Int(_))) => Err(unsupported("Flag members whose values are not ints")),
        _ => Ok(()),
    }
}
fn flag_int(v: &V) -> i64 {
    match v {
        V::Obj(o) => match o.get("_value_") {
            Some(V::Int(i)) => i,
            _ => 0,
        },
        V::Int(i) => *i,
        V::Bool(b) => *b as i64,
        _ => 0,
    }
}
fn cls_int(c: &Rc<Class>, n: &str) -> i64 {
    match c.lookup(n) {
        Some(V::Int(i)) => i,
        _ => 0,
    }
}
// The members in `value`'s bits: by value, or in definition order when the members' values are not sorted.
fn flag_iter(c: &Rc<Class>, value: i64) -> Result<Vec<V>, E> {
    let v2m = c.lookup("_value2member_map_").unwrap_or(V::None);
    let mut out = vec![];
    let mut n = value & cls_int(c, "_flag_mask_");
    while n != 0 {
        let b = n & n.wrapping_neg();
        out.push(match &v2m {
            V::Dict(d) => Dict::get_cell(d, &V::Int(b))?.unwrap_or(V::None),
            _ => V::None,
        });
        n ^= b;
    }
    if matches!(c.lookup("__bv_bydef__"), Some(V::Bool(true))) {
        let order = enum_members(c);
        let pos = |m: &V| -> usize {
            match &order {
                V::List(l) => l.borrow().iter().position(|x| is(x, m)).unwrap_or(usize::MAX),
                _ => usize::MAX,
            }
        };
        out.sort_by_key(|m| pos(m));
    }
    Ok(out)
}
fn flag_bin(num: i64, max_bits: usize) -> String {
    // enum.bin: two's complement with a sign digit
    let bits = 64 - (if num >= 0 { num as u64 } else { !(num as u64) }).leading_zeros() as usize;
    let ceiling: i128 = 1i128 << bits;
    let body = if num >= 0 { format!("{:b}", num as i128 + ceiling) } else { format!("{:b}", (!(num as i128) ^ (ceiling - 1)) + ceiling) };
    let (sign, mut digits) = if num >= 0 { ("0".to_string(), body[1..].to_string()) } else { ("1".to_string(), body[1..].to_string()) };
    if digits.len() < max_bits {
        let pad: String = sign.repeat(max_bits);
        digits = format!("{pad}{digits}");
        digits = digits[digits.len() - max_bits..].to_string();
    }
    format!("0b{sign} {digits}")
}
// Flag's _missing_: the member for `value`, made (and kept) as CPython makes it.
fn flag_member(c: &Rc<Class>, value: i64) -> R {
    let v2m = match c.lookup("_value2member_map_") {
        Some(V::Dict(d)) => d,
        _ => return val_err(format!("{value} is not a valid {}", c.name)),
    };
    if let Some(m) = Dict::get_cell(&v2m, &V::Int(value))? {
        return Ok(m);
    }
    let (fm, sm, all) = (cls_int(c, "_flag_mask_"), cls_int(c, "_singles_mask_"), cls_int(c, "_all_bits_"));
    let keep = c.lookup("__bv_keep__").is_some();
    let mut value = value;
    let mut neg = None;
    if !(!all <= value && value <= all) || value & (all ^ fm) != 0 {
        if !keep {
            let bl = |x: i64| 64 - (if x >= 0 { x as u64 } else { !(x as u64) }).leading_zeros() as usize;
            let mb = bl(value).max(bl(fm));
            return val_err(format!("<flag '{}'> invalid value {}\n    given {}\n  allowed {}", c.name, value, flag_bin(value, mb), flag_bin(fm, mb)));
        }
        if value < 0 {
            let bl = 64 - (!(value as u64)).leading_zeros();
            if bl >= 62 {
                return Err(unsupported("IntFlag values past 2**62"));
            }
            value += (all + 1).max(1i64 << bl);
        }
    }
    if value < 0 {
        neg = Some(value);
        value = all + 1 + value;
    }
    let unknown0 = value & !fm;
    let aliases = value & !sm;
    let member_value = value & sm;
    if unknown0 != 0 && !keep {
        return val_err(format!("{}({value}) -->  unknown values {unknown0} [{}]", c.name, bin_text(unknown0)));
    }
    let o = Obj::new(c.clone());
    o.set("_value_", V::Int(value));
    o.set("value", V::Int(value));
    if !matches!(c.base_bt(), BT::Object | BT::Other) {
        o.set_inner(Some(V::Int(value)));
    }
    let mut name = V::None;
    if member_value != 0 || aliases != 0 {
        let mut members = flag_iter(c, member_value)?;
        let mut combined = members.iter().fold(0, |a, m| a | flag_int(m));
        let mut value2 = value;
        if aliases != 0 {
            value2 = member_value | aliases;
            if let Some(V::Dict(map)) = c.own("_member_map_") {
                for (_, pm) in map.borrow().items() {
                    let pv = flag_int(&pm);
                    if !members.iter().any(|m| is(m, &pm)) && pv != 0 && pv & value2 == pv {
                        combined |= pv;
                        members.push(pm);
                    }
                }
            }
        }
        let unknown = value2 ^ combined;
        let mut names: Vec<String> = vec![];
        for m in &members {
            if let V::Obj(mo) = m {
                names.push(to_str(&mo.get("_name_").unwrap_or(V::None))?);
            }
        }
        if combined == 0 {
            name = V::None;
        } else if unknown != 0 && !keep {
            return val_err(format!("<flag '{}'>: no members with value {unknown}", c.name));
        } else if unknown != 0 {
            name = st(format!("{}|{unknown}", names.join("|")));
        } else {
            name = st(names.join("|"));
        }
    }
    o.set("_name_", name.clone());
    o.set("name", name);
    let m = V::Obj(o);
    Dict::set_cell(&v2m, V::Int(value), m.clone())?;
    if let Some(n) = neg {
        Dict::set_cell(&v2m, V::Int(n), m.clone())?;
    }
    Ok(m)
}
fn bin_text(n: i64) -> String {
    if n < 0 {
        format!("-0b{:b}", n.unsigned_abs())
    } else {
        format!("0b{n:b}")
    }
}
fn flag_methods(fl: &Rc<Class>) {
    fn me_cls(a: &Args) -> Result<Rc<Class>, E> {
        match a.p.first() {
            Some(V::Obj(o)) => Ok(o.cls.clone()),
            _ => type_err("descriptor needs a Flag member".into()),
        }
    }
    // _get_value: a member of the same class, or (IntFlag) an int
    fn other_val(c: &Rc<Class>, x: &V) -> Option<i64> {
        match x {
            V::Obj(o) if o.cls.is_sub(c) => Some(flag_int(x)),
            V::Int(i) if !matches!(c.base_bt(), BT::Object | BT::Other) => Some(*i),
            V::Bool(b) if !matches!(c.base_bt(), BT::Object | BT::Other) => Some(*b as i64),
            _ => None,
        }
    }
    for (name, op) in [("__or__", '|'), ("__ror__", '|'), ("__and__", '&'), ("__rand__", '&'), ("__xor__", '^'), ("__rxor__", '^')] {
        fl.set(name, builtin(name, move |a: Args| {
            let c = me_cls(&a)?;
            let Some(o) = a.p.get(1).and_then(|x| other_val(&c, x)) else { return Ok(V::NotImpl) };
            int_flag_value(&a.p[0])?;
            int_flag_value(&a.p[1])?;
            let v = flag_int(&a.p[0]);
            flag_member(&c, match op {
                '|' => v | o,
                '&' => v & o,
                _ => v ^ o,
            })
        }));
    }
    fl.set("__invert__", builtin("__invert__", |a: Args| {
        let c = me_cls(&a)?;
        int_flag_value(&a.p[0])?;
        let v = flag_int(&a.p[0]);
        if c.lookup("__bv_keep__").is_some() {
            flag_member(&c, !v)
        } else {
            flag_member(&c, cls_int(&c, "_singles_mask_") & !v)
        }
    }));
    fl.set("__contains__", builtin("__contains__", |a: Args| {
        let c = me_cls(&a)?;
        let other = a.p.get(1).cloned().unwrap_or(V::None);
        match &other {
            V::Obj(o) if o.cls.is_sub(&c) => {
                int_flag_value(&a.p[0])?;
                int_flag_value(&other)?;
                let ov = flag_int(&other);
                Ok(V::Bool(ov & flag_int(&a.p[0]) == ov))
            }
            x => type_err(format!("unsupported operand type(s) for 'in': '{}' and '{}'", tn(x), c.name)),
        }
    }));
    fl.set("__iter__", builtin("__iter__", |a: Args| {
        let c = me_cls(&a)?;
        Ok(list(flag_iter(&c, flag_int(&a.p[0]))?))
    }));
    fl.set("__len__", builtin("__len__", |a: Args| Ok(V::Int(flag_int(&a.p[0]).count_ones() as i64))));
    fl.set("__bool__", builtin("__bool__", |a: Args| Ok(V::Bool(flag_int(&a.p[0]) != 0))));
    fl.set("__repr__", builtin("__repr__", |a: Args| {
        let c = me_cls(&a)?;
        let V::Obj(o) = &a.p[0] else { return Ok(s("")) };
        let v = repr(&o.get("_value_").unwrap_or(V::None))?;
        Ok(st(match o.get("_name_") {
            Some(V::Str(n)) => format!("<{}.{}: {v}>", c.name, n.s),
            _ => format!("<{}: {v}>", c.name),
        }))
    }));
    fl.set("__str__", builtin("__str__", |a: Args| {
        let c = me_cls(&a)?;
        let V::Obj(o) = &a.p[0] else { return Ok(s("")) };
        Ok(st(match o.get("_name_") {
            Some(V::Str(n)) => format!("{}.{}", c.name, n.s),
            _ => format!("{}({})", c.name, repr(&o.get("_value_").unwrap_or(V::None))?),
        }))
    }));
}
pub fn enum_call(c: &Rc<Class>, a: Args) -> R {
    if !a.k.is_empty() || a.p.len() != 1 {
        return Err(unsupported("calling an Enum class with other than one value"));
    }
    let v = &a.p[0];
    if let V::List(l) = enum_members(c) {
        for m in l.borrow().iter() {
            if is(m, v) {
                return Ok(m.clone());
            }
        }
        for m in l.borrow().iter() {
            if let V::Obj(o) = m {
                if eq(&o.get("_value_").unwrap_or(V::None), v)? {
                    return Ok(m.clone());
                }
            }
        }
    }
    if is_flag(c) {
        match v {
            V::Int(i) => return flag_member(c, *i),
            V::Bool(b) => return flag_member(c, *b as i64),
            V::Big(_) => return Err(unsupported("Flag values past 2**62")),
            _ => {}
        }
    }
    val_err(format!("{} is not a valid {}", repr(v)?, c.name))
}
pub fn enum_getitem(c: &Rc<Class>, k: &V) -> R {
    if let Some(V::Dict(d)) = c.own("__members__") {
        if let Some(m) = Dict::get_cell(&d, k)? {
            return Ok(m);
        }
    }
    Err(exc_v("KeyError", k.clone()))
}
fn is_val_err(e: &E) -> bool {
    matches!(&e.0, V::Obj(o) if o.cls.name == "ValueError")
}
// EnumType.__contains__: an instance of the class is in it; a Flag also accepts any value its _missing_
// turns into a member (pseudo-members included); finally a value equal to a member's value is in it.
pub fn enum_contains(c: &Rc<Class>, x: &V) -> Result<bool, E> {
    if let V::Obj(o) = x {
        if o.cls.is_sub(c) {
            return Ok(true);
        }
    }
    if is_flag(c) {
        let keep = c.lookup("__bv_keep__").is_some();
        let v = match x {
            V::Int(i) => Some(*i),
            V::Bool(b) => Some(*b as i64),
            V::Obj(_) => match inner_of(x) {
                Some(V::Int(i)) => Some(i),
                Some(V::Bool(b)) => Some(b as i64),
                Some(V::Big(_)) if keep => return Err(unsupported("IntFlag values past 2**62")),
                _ => None,
            },
            V::Big(_) if keep => return Err(unsupported("IntFlag values past 2**62")),
            _ => None,
        };
        if let Some(v) = v {
            // _missing_ raises ValueError for values it cannot make a member of; EnumType.__contains__ then
            // falls through to the member-value check below (and returns False there).
            return match flag_member(c, v) {
                Ok(_) => Ok(true),
                Err(e) if is_val_err(&e) => Ok(false),
                Err(e) => Err(e),
            };
        }
    }
    if let V::List(l) = enum_members(c) {
        for m in l.borrow().iter() {
            if is(m, x) {
                return Ok(true);
            }
            if let V::Obj(o) = m {
                if eq(&o.get("_value_").unwrap_or(V::None), x)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
pub fn enum_fn(n: &str, a: Args) -> R {
    match n {
        "auto" => Ok(V::Obj(Obj::new(enum_classes().1))),
        "unique" => {
            let c = a.p.first().cloned().unwrap_or(V::None);
            if let V::Class(k) = &c {
                if let (V::List(l), Some(V::Dict(d))) = (enum_members(k), k.own("__members__")) {
                    let ms = l.borrow().clone();
                    let mut dups = vec![];
                    for (name, m) in d.borrow().items() {
                        if let (V::Str(name), V::Obj(o)) = (&name, &m) {
                            if let Some(V::Str(real)) = o.get("_name_") {
                                if real.s != name.s && ms.iter().any(|x| is(x, &m)) {
                                    dups.push(format!("{} -> {}", name.s, real.s));
                                }
                            }
                        }
                    }
                    if !dups.is_empty() {
                        return val_err(format!("duplicate values found in <enum '{}'>: {}", k.name, dups.join(", ")));
                    }
                }
            }
            Ok(c)
        }
        _ => Err(unsupported(&format!("enum.{n}"))),
    }
}
// ---------- re.RegexFlag ----------
// re.IGNORECASE and the rest are members of an IntFlag, as in CPython, so they print as `re.IGNORECASE` and
// combine as `re.IGNORECASE|re.DOTALL`, while still working as ints.
const RE_FLAGS: [(Sym, Sym, i64); 9] = [
    ("NOFLAG", "", 0),
    ("ASCII", "A", 256),
    ("IGNORECASE", "I", 2),
    ("LOCALE", "L", 4),
    ("UNICODE", "U", 32),
    ("MULTILINE", "M", 8),
    ("DOTALL", "S", 16),
    ("VERBOSE", "X", 64),
    ("DEBUG", "", 128),
];
thread_local! {
    static RE_FLAG: RefCell<Option<Rc<Class>>> = RefCell::new(None);
}
// CPython's re.global_flag_repr over a flags value: the members in definition order, then the unknown bits in hex
pub fn re_flag_names(v: i64) -> String {
    if v == 0 {
        return "re.NOFLAG".into();
    }
    let mut parts = vec![];
    let mut rest = v;
    for (n, _, b) in RE_FLAGS {
        if b != 0 && v & b == b {
            parts.push(format!("re.{n}"));
            rest &= !b;
        }
    }
    if rest > 0 {
        parts.push(format!("{rest:#x}"));
    } else if rest < 0 {
        parts.push(format!("-{:#x}", rest.unsigned_abs()));
    }
    parts.join("|")
}
pub fn re_flag_class() -> Rc<Class> {
    if let Some(c) = RE_FLAG.with(|r| r.borrow().clone()) {
        return c;
    }
    let ifl = enum_more()[3].clone();
    let c = Class::new("RegexFlag", "re", vec![ifl], BT::Other);
    for (n, a, b) in RE_FLAGS {
        c.set(n, V::Int(b));
        if !a.is_empty() {
            c.set(a, V::Int(b));
        }
    }
    let rep = builtin("__repr__", |a: Args| {
        let v = match a.p.first() {
            Some(V::Obj(o)) => o.get("_value_").unwrap_or(V::Int(0)),
            Some(x) => x.clone(),
            None => V::Int(0),
        };
        match v {
            V::Int(i) => Ok(s(&re_flag_names(i))),
            x => Ok(s(&repr(&x)?)),
        }
    });
    c.set("__repr__", rep.clone());
    c.set("__str__", rep);
    let _ = enum_setup(&c);
    RE_FLAG.with(|r| *r.borrow_mut() = Some(c.clone()));
    c
}
// re.IGNORECASE, re.I, ..., re.RegexFlag
pub fn re_flag(n: &str) -> Option<V> {
    if n == "RegexFlag" {
        return Some(V::Class(re_flag_class()));
    }
    // re.LOCALE is for bytes patterns, which Brevi's re does not match: a program using it is refused
    if n == "LOCALE" || n == "L" || !RE_FLAGS.iter().any(|(a, b, _)| *a == n || *b == n) {
        return None;
    }
    re_flag_class().lookup(n)
}
