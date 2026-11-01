//! brevi_tk: the GUI runtime Brevi compiles Python `tkinter` programs onto.
//!
//! The program builds a tree of widgets through handles (`W`, a plain index), much as tkinter does, and
//! hands control to `mainloop`. Two front ends draw the same tree:
//! - a window, through egui (feature `window`), when `BREVI_GUI` is not `headless`;
//! - headless, for tests and for AI models that cannot see a screen: `mainloop` reads actions from stdin
//!   (`click +1`, `type 1 Adam`, `show`, ...) and prints what the window would show. See `headless`.
//!
//! Options are passed as name/value pairs, as tkinter's keyword arguments are, so the compiler can hand
//! them over as written: `Label(root, text="Hi", fg="red")` is `widget("Label", root, vec![("text", "Hi".into()), ...])`.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

mod headless;
#[cfg(feature = "window")]
mod window;

/// A widget handle (index into the widget table). The root window is 0.
pub type W = usize;
/// A tkinter variable handle (`StringVar`, `IntVar`, `BooleanVar`, `DoubleVar`).
pub type V = usize;

/// A callback with no arguments (`command=`, `after`).
pub type Cb = Rc<dyn Fn()>;
/// A callback given the event (`bind`).
pub type EvCb = Rc<dyn Fn(&Event)>;

/// What a `bind` callback gets: the widget, the key for key events, and the pointer position.
#[derive(Clone, Debug, Default)]
pub struct Event {
    pub widget: W,
    pub keysym: String,
    pub char: String,
    pub x: i64,
    pub y: i64,
}

/// An option value.
#[derive(Clone)]
pub enum Val {
    None,
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Var(V),
    Cb(Cb),
    List(Vec<Val>),
}

impl std::fmt::Debug for Val {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Val::Cb(_) => write!(f, "<callback>"),
            v => write!(f, "{}", v.text()),
        }
    }
}

impl Val {
    /// The value as tkinter shows it in a widget.
    pub fn text(&self) -> String {
        match self {
            Val::None => String::new(),
            Val::Str(s) => s.clone(),
            Val::Int(i) => i.to_string(),
            Val::Float(x) => {
                let s = format!("{x}");
                if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") { s } else { format!("{s}.0") }
            }
            Val::Bool(b) => (if *b { "1" } else { "0" }).into(),
            Val::Var(v) => var_get(*v).text(),
            Val::Cb(_) => String::new(),
            Val::List(xs) => xs.iter().map(|x| x.text()).collect::<Vec<_>>().join(" "),
        }
    }
    pub fn int(&self) -> i64 {
        match self {
            Val::Int(i) => *i,
            Val::Float(x) => *x as i64,
            Val::Bool(b) => *b as i64,
            Val::Str(s) => s.trim().parse::<f64>().map(|x| x as i64).unwrap_or(0),
            Val::Var(v) => var_get(*v).int(),
            _ => 0,
        }
    }
    pub fn float(&self) -> f64 {
        match self {
            Val::Int(i) => *i as f64,
            Val::Float(x) => *x,
            Val::Bool(b) => *b as i64 as f64,
            Val::Str(s) => s.trim().parse().unwrap_or(0.0),
            Val::Var(v) => var_get(*v).float(),
            _ => 0.0,
        }
    }
    pub fn truthy(&self) -> bool {
        match self {
            Val::None => false,
            Val::Str(s) => !s.is_empty() && s != "0",
            Val::Int(i) => *i != 0,
            Val::Float(x) => *x != 0.0,
            Val::Bool(b) => *b,
            Val::Var(v) => var_get(*v).truthy(),
            Val::Cb(_) => true,
            Val::List(xs) => !xs.is_empty(),
        }
    }
}

impl From<&str> for Val { fn from(s: &str) -> Self { Val::Str(s.to_string()) } }
impl From<String> for Val { fn from(s: String) -> Self { Val::Str(s) } }
impl From<&String> for Val { fn from(s: &String) -> Self { Val::Str(s.clone()) } }
impl From<i64> for Val { fn from(i: i64) -> Self { Val::Int(i) } }
impl From<i32> for Val { fn from(i: i32) -> Self { Val::Int(i as i64) } }
impl From<usize> for Val { fn from(i: usize) -> Self { Val::Int(i as i64) } }
impl From<f64> for Val { fn from(x: f64) -> Self { Val::Float(x) } }
impl From<bool> for Val { fn from(b: bool) -> Self { Val::Bool(b) } }

/// Widget kinds. `Tk` and `Toplevel` are windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Tk, Toplevel, Frame, LabelFrame, Label, Button, Entry, Text, Checkbutton, Radiobutton, Listbox, Scale, Spinbox, Canvas, Scrollbar, Combobox, Progressbar, Separator }

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "Tk" => Kind::Tk,
            "Toplevel" => Kind::Toplevel,
            "Frame" => Kind::Frame,
            "LabelFrame" | "Labelframe" => Kind::LabelFrame,
            "Label" | "Message" => Kind::Label,
            "Button" => Kind::Button,
            "Entry" => Kind::Entry,
            "Text" | "ScrolledText" => Kind::Text,
            "Checkbutton" => Kind::Checkbutton,
            "Radiobutton" => Kind::Radiobutton,
            "Listbox" => Kind::Listbox,
            "Scale" => Kind::Scale,
            "Spinbox" => Kind::Spinbox,
            "Canvas" => Kind::Canvas,
            "Scrollbar" => Kind::Scrollbar,
            "Combobox" | "OptionMenu" => Kind::Combobox,
            "Progressbar" => Kind::Progressbar,
            "Separator" => Kind::Separator,
            _ => return None,
        })
    }
}

/// How a widget is placed in its parent.
#[derive(Clone, Debug, Default)]
pub enum Place {
    /// Not placed yet: tkinter does not show it.
    #[default]
    None,
    Pack { side: String, fill: String, expand: bool, padx: i64, pady: i64 },
    Grid { row: i64, column: i64, columnspan: i64, rowspan: i64, sticky: String, padx: i64, pady: i64 },
    Place { x: i64, y: i64 },
}

/// One canvas item (`create_rectangle` and friends).
#[derive(Clone, Debug)]
pub struct Item {
    pub id: i64,
    pub kind: String,
    pub coords: Vec<f64>,
    pub opts: BTreeMap<String, Val>,
}

pub struct Widget {
    pub kind: Kind,
    pub parent: Option<W>,
    pub children: Vec<W>,
    pub opts: BTreeMap<String, Val>,
    pub place: Place,
    pub alive: bool,
    /// Entry, Text, Spinbox and Combobox contents (when no textvariable).
    pub text: String,
    /// Listbox and Combobox lines.
    pub lines: Vec<String>,
    pub selection: Vec<usize>,
    pub items: Vec<Item>,
    pub binds: Vec<(String, EvCb)>,
    /// Checkbutton and Radiobutton state without a variable.
    pub checked: bool,
    pub title: String,
}

pub(crate) struct Var {
    pub(crate) kind: &'static str,
    pub(crate) val: Val,
    pub(crate) traces: Vec<Cb>,
}

struct Timer {
    due: u64,
    seq: u64,
    id: i64,
    cb: Cb,
}

#[derive(Default)]
pub(crate) struct App {
    pub(crate) widgets: Vec<Widget>,
    pub(crate) vars: Vec<Var>,
    timers: Vec<Timer>,
    now_ms: u64,
    seq: u64,
    next_item: i64,
    quit: bool,
    /// Answers for dialogs in headless mode, in order (`answer yes`).
    answers: Vec<String>,
}

thread_local! {
    pub(crate) static APP: RefCell<App> = RefCell::new(App::default());
}

fn with<R>(f: impl FnOnce(&mut App) -> R) -> R {
    APP.with(|a| f(&mut a.borrow_mut()))
}

fn opts_map(opts: Vec<(&str, Val)>) -> BTreeMap<String, Val> {
    opts.into_iter().map(|(k, v)| (k.trim_end_matches('_').to_string(), v)).collect()
}

/// `tk.Tk()`: the root window (always handle 0).
pub fn tk() -> W {
    with(|a| {
        if a.widgets.is_empty() {
            a.widgets.push(new_widget(Kind::Tk, None, BTreeMap::new()));
            a.widgets[0].title = "tk".into();
        }
        0
    })
}

fn new_widget(kind: Kind, parent: Option<W>, opts: BTreeMap<String, Val>) -> Widget {
    Widget {
        kind,
        parent,
        children: vec![],
        opts,
        place: Place::None,
        alive: true,
        text: String::new(),
        lines: vec![],
        selection: vec![],
        items: vec![],
        binds: vec![],
        checked: false,
        title: String::new(),
    }
}

/// `tk.Label(parent, text=...)` and every other widget class, by name. Unknown names panic with a clear
/// message (the compiler only emits names `Kind::parse` accepts).
pub fn widget(kind: &str, parent: W, opts: Vec<(&str, Val)>) -> W {
    let k = Kind::parse(kind).unwrap_or_else(|| panic!("tkinter widget {kind} is not supported"));
    tk();
    with(|a| {
        let id = a.widgets.len();
        let mut w = new_widget(k, Some(parent), opts_map(opts));
        if let Some(Val::List(vs)) = w.opts.get("values") {
            w.lines = vs.iter().map(|v| v.text()).collect();
        }
        if k == Kind::Toplevel {
            w.title = "tk".into();
        }
        a.widgets.push(w);
        a.widgets[parent].children.push(id);
        id
    })
}

/// `w.config(text=...)` / `w.configure(...)`; `w["text"] = ...` too.
pub fn config(w: W, opts: Vec<(&str, Val)>) {
    with(|a| {
        for (k, v) in opts_map(opts) {
            if k == "values" {
                if let Val::List(vs) = &v {
                    a.widgets[w].lines = vs.iter().map(|v| v.text()).collect();
                }
            }
            a.widgets[w].opts.insert(k, v);
        }
    })
}

/// `w.cget("text")` / `w["text"]`.
pub fn cget(w: W, key: &str) -> Val {
    with(|a| a.widgets[w].opts.get(key).cloned().unwrap_or(Val::None))
}

pub fn title(w: W, t: &str) {
    with(|a| a.widgets[w].title = t.to_string())
}

/// `geometry("300x200")` and `resizable(...)`, `minsize(...)`: kept, used by the window front end only.
pub fn geometry(w: W, g: &str) {
    with(|a| {
        a.widgets[w].opts.insert("geometry".into(), Val::Str(g.into()));
    })
}

fn int_opt(o: &BTreeMap<String, Val>, k: &str, d: i64) -> i64 {
    match o.get(k) {
        Some(Val::List(xs)) => xs.first().map(|x| x.int()).unwrap_or(d),
        Some(v) => v.int(),
        None => d,
    }
}

pub fn pack(w: W, opts: Vec<(&str, Val)>) {
    let o = opts_map(opts);
    let s = |k: &str, d: &str| o.get(k).map(|v| v.text()).unwrap_or_else(|| d.to_string());
    let p = Place::Pack { side: s("side", "top"), fill: s("fill", "none"), expand: o.get("expand").map(|v| v.truthy()).unwrap_or(false), padx: int_opt(&o, "padx", 0), pady: int_opt(&o, "pady", 0) };
    with(|a| a.widgets[w].place = p);
}

pub fn grid(w: W, opts: Vec<(&str, Val)>) {
    let o = opts_map(opts);
    let p = Place::Grid {
        row: int_opt(&o, "row", 0),
        column: int_opt(&o, "column", 0),
        columnspan: int_opt(&o, "columnspan", 1),
        rowspan: int_opt(&o, "rowspan", 1),
        sticky: o.get("sticky").map(|v| v.text()).unwrap_or_default(),
        padx: int_opt(&o, "padx", 0),
        pady: int_opt(&o, "pady", 0),
    };
    with(|a| a.widgets[w].place = p);
}

pub fn place(w: W, opts: Vec<(&str, Val)>) {
    let o = opts_map(opts);
    let p = Place::Place { x: int_opt(&o, "x", 0), y: int_opt(&o, "y", 0) };
    with(|a| a.widgets[w].place = p);
}

pub fn pack_forget(w: W) {
    with(|a| a.widgets[w].place = Place::None)
}

/// `w.destroy()`: the widget and its children go away; destroying the root ends `mainloop`.
pub fn destroy(w: W) {
    with(|a| {
        let mut stack = vec![w];
        while let Some(x) = stack.pop() {
            a.widgets[x].alive = false;
            stack.extend(a.widgets[x].children.iter().copied());
        }
        if w == 0 {
            a.quit = true;
        }
    })
}

/// `root.quit()`.
pub fn quit() {
    with(|a| a.quit = true)
}

// ---------------------------------------------------------------- variables

/// `tk.StringVar(value=...)`, `IntVar`, `BooleanVar`, `DoubleVar`.
pub fn var(kind: &str, value: Option<Val>) -> V {
    let kind: &'static str = match kind {
        "IntVar" => "IntVar",
        "BooleanVar" => "BooleanVar",
        "DoubleVar" => "DoubleVar",
        _ => "StringVar",
    };
    let init = value.unwrap_or(match kind {
        "IntVar" => Val::Int(0),
        "BooleanVar" => Val::Bool(false),
        "DoubleVar" => Val::Float(0.0),
        _ => Val::Str(String::new()),
    });
    with(|a| {
        a.vars.push(Var { kind, val: coerce(kind, &init), traces: vec![] });
        a.vars.len() - 1
    })
}

fn coerce(kind: &str, v: &Val) -> Val {
    match kind {
        "IntVar" => Val::Int(v.int()),
        "BooleanVar" => Val::Bool(v.truthy()),
        "DoubleVar" => Val::Float(v.float()),
        _ => Val::Str(v.text()),
    }
}

pub fn var_get(v: V) -> Val {
    with(|a| a.vars[v].val.clone())
}

pub fn var_get_str(v: V) -> String { var_get(v).text() }
pub fn var_get_int(v: V) -> i64 { var_get(v).int() }
pub fn var_get_float(v: V) -> f64 { var_get(v).float() }
pub fn var_get_bool(v: V) -> bool { var_get(v).truthy() }

/// `v.set(x)`; runs `trace_add("write", ...)` callbacks.
pub fn var_set(v: V, x: impl Into<Val>) {
    let x = x.into();
    let traces = with(|a| {
        let k = a.vars[v].kind;
        a.vars[v].val = coerce(k, &x);
        a.vars[v].traces.clone()
    });
    for t in traces {
        t();
    }
}

pub fn var_trace(v: V, cb: Cb) {
    with(|a| a.vars[v].traces.push(cb))
}

// ---------------------------------------------------------------- widget contents

fn textvar(a: &App, w: W) -> Option<V> {
    match a.widgets[w].opts.get("textvariable") {
        Some(Val::Var(v)) => Some(*v),
        _ => None,
    }
}

/// `entry.get()`, `spinbox.get()`, `combobox.get()`; `text.get("1.0", "end")` uses `text_get`.
pub fn get(w: W) -> String {
    let (tv, kind) = with(|a| (textvar(a, w), a.widgets[w].kind));
    if let Some(v) = tv {
        return var_get_str(v);
    }
    with(|a| match kind {
        Kind::Scale => a.widgets[w].opts.get("value").map(|v| v.text()).unwrap_or_else(|| "0".into()),
        _ => a.widgets[w].text.clone(),
    })
}

/// Sets the whole text (used by `insert`, `delete` and the front ends).
pub fn set_text(w: W, s: &str) {
    let tv = with(|a| textvar(a, w));
    match tv {
        Some(v) => var_set(v, s),
        None => with(|a| a.widgets[w].text = s.to_string()),
    }
}

/// tkinter text indexes: `0`, `"end"`, `tk.END`, `"1.0"` (line.column, for Text), `"insert"`.
fn index(s: &str, idx: &Val) -> usize {
    let n = s.chars().count();
    match idx {
        Val::Int(i) => (*i).clamp(0, n as i64) as usize,
        Val::Str(t) if t == "end" || t == "insert" || t.starts_with("end") => n,
        Val::Str(t) if t.contains('.') => {
            let mut it = t.split('.');
            let line: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or(1);
            let col = it.next().unwrap_or("0");
            let lines: Vec<&str> = s.split('\n').collect();
            let mut off = 0;
            for l in lines.iter().take(line.saturating_sub(1).min(lines.len())) {
                off += l.chars().count() + 1;
            }
            let this = lines.get(line.saturating_sub(1)).map(|l| l.chars().count()).unwrap_or(0);
            let c = if col == "end" { this } else { col.parse().unwrap_or(0).min(this) };
            (off + c).min(n)
        }
        v => (v.int().max(0) as usize).min(n),
    }
}

/// `entry.insert(i, s)`, `text.insert("end", s)`, `listbox.insert("end", s)`.
pub fn insert(w: W, at: Val, s: impl Into<Val>) {
    let s = s.into().text();
    let kind = with(|a| a.widgets[w].kind);
    if kind == Kind::Listbox || kind == Kind::Combobox {
        with(|a| {
            let n = a.widgets[w].lines.len();
            let i = match &at { Val::Str(t) if t.starts_with("end") => n, v => (v.int().max(0) as usize).min(n) };
            a.widgets[w].lines.insert(i, s);
        });
        return;
    }
    if kind == Kind::Canvas {
        return;
    }
    let cur = get(w);
    let i = index(&cur, &at);
    let mut cs: Vec<char> = cur.chars().collect();
    let ins: Vec<char> = s.chars().collect();
    cs.splice(i..i, ins);
    set_text(w, &cs.into_iter().collect::<String>());
}

/// `entry.delete(0, tk.END)`, `listbox.delete(i)`, `canvas.delete("all")` / `canvas.delete(item)`.
pub fn delete(w: W, a0: Val, a1: Option<Val>) {
    let kind = with(|a| a.widgets[w].kind);
    match kind {
        Kind::Canvas => with(|a| match &a0 {
            Val::Str(t) if t == "all" => a.widgets[w].items.clear(),
            v => {
                let id = v.int();
                let tag = v.text();
                a.widgets[w].items.retain(|it| it.id != id && it.opts.get("tags").map(|t| t.text()) != Some(tag.clone()));
            }
        }),
        Kind::Listbox | Kind::Combobox => with(|a| {
            let n = a.widgets[w].lines.len();
            let ix = |v: &Val| match v { Val::Str(t) if t.starts_with("end") => n.saturating_sub(1), v => v.int().max(0) as usize };
            let i = ix(&a0);
            let j = a1.as_ref().map(ix).unwrap_or(i);
            if i < n {
                let j = j.min(n - 1);
                if j >= i {
                    a.widgets[w].lines.drain(i..=j);
                }
            }
            a.widgets[w].selection.clear();
        }),
        _ => {
            let cur = get(w);
            let i = index(&cur, &a0);
            let j = match &a1 { Some(v) => index(&cur, v), None => (i + 1).min(cur.chars().count()) };
            let mut cs: Vec<char> = cur.chars().collect();
            if i < j {
                cs.drain(i..j);
            }
            set_text(w, &cs.into_iter().collect::<String>());
        }
    }
}

/// `text.get("1.0", "end")` (tkinter adds a final newline for "end", as here).
pub fn text_get(w: W, a0: Val, a1: Option<Val>) -> String {
    let cur = get(w);
    let i = index(&cur, &a0);
    let (j, nl) = match &a1 {
        Some(Val::Str(t)) if t.starts_with("end") && t != "end-1c" => (cur.chars().count(), true),
        Some(v) => (index(&cur, v), false),
        None => ((i + 1).min(cur.chars().count()), false),
    };
    let mut s: String = cur.chars().skip(i).take(j.saturating_sub(i)).collect();
    if nl {
        s.push('\n');
    }
    s
}

/// `listbox.get(i)`.
pub fn line(w: W, i: i64) -> String {
    with(|a| a.widgets[w].lines.get(i.max(0) as usize).cloned().unwrap_or_default())
}

/// `listbox.size()`.
pub fn size(w: W) -> i64 {
    with(|a| a.widgets[w].lines.len() as i64)
}

/// `listbox.curselection()`.
pub fn curselection(w: W) -> Vec<i64> {
    with(|a| a.widgets[w].selection.iter().map(|&i| i as i64).collect())
}

pub fn selection_set(w: W, i: i64) {
    with(|a| {
        let i = i.max(0) as usize;
        if i < a.widgets[w].lines.len() && !a.widgets[w].selection.contains(&i) {
            a.widgets[w].selection.push(i);
            a.widgets[w].selection.sort();
        }
    })
}

pub fn selection_clear(w: W) {
    with(|a| a.widgets[w].selection.clear())
}

/// Checkbutton/Radiobutton state, from its variable when it has one.
pub fn is_checked(w: W) -> bool {
    let (var, on, val, kind, own) = with(|a| {
        let x = &a.widgets[w];
        let var = match x.opts.get("variable") { Some(Val::Var(v)) => Some(*v), _ => None };
        (var, x.opts.get("onvalue").cloned(), x.opts.get("value").cloned(), x.kind, x.checked)
    });
    match (var, kind) {
        (Some(v), Kind::Radiobutton) => var_get(v).text() == val.map(|x| x.text()).unwrap_or_default(),
        (Some(v), _) => match on { Some(o) => var_get(v).text() == o.text(), None => var_get(v).truthy() },
        (None, _) => own,
    }
}

/// `scale.get()` / `scale.set(x)`.
pub fn scale_get(w: W) -> f64 {
    let var = with(|a| match a.widgets[w].opts.get("variable") { Some(Val::Var(v)) => Some(*v), _ => None });
    match var {
        Some(v) => var_get_float(v),
        None => with(|a| a.widgets[w].opts.get("value").map(|v| v.float()).unwrap_or_else(|| a.widgets[w].opts.get("from").map(|v| v.float()).unwrap_or(0.0))),
    }
}

pub fn scale_set(w: W, x: f64) {
    let var = with(|a| match a.widgets[w].opts.get("variable") { Some(Val::Var(v)) => Some(*v), _ => None });
    match var {
        Some(v) => var_set(v, x),
        None => with(|a| {
            a.widgets[w].opts.insert("value".into(), Val::Float(x));
        }),
    }
}

// ---------------------------------------------------------------- canvas

/// `canvas.create_rectangle(x1, y1, x2, y2, fill="red")` and the other `create_*` methods; returns the item id.
pub fn create(w: W, kind: &str, coords: Vec<f64>, opts: Vec<(&str, Val)>) -> i64 {
    with(|a| {
        a.next_item += 1;
        let id = a.next_item;
        a.widgets[w].items.push(Item { id, kind: kind.to_string(), coords, opts: opts_map(opts) });
        id
    })
}

/// `canvas.coords(item, x1, y1, ...)` (empty `xs` reads them).
pub fn coords(w: W, id: i64, xs: Vec<f64>) -> Vec<f64> {
    with(|a| match a.widgets[w].items.iter_mut().find(|it| it.id == id) {
        Some(it) => {
            if !xs.is_empty() {
                it.coords = xs;
            }
            it.coords.clone()
        }
        None => vec![],
    })
}

pub fn move_item(w: W, id: i64, dx: f64, dy: f64) {
    with(|a| {
        for it in a.widgets[w].items.iter_mut().filter(|it| it.id == id) {
            for (k, c) in it.coords.iter_mut().enumerate() {
                *c += if k % 2 == 0 { dx } else { dy };
            }
        }
    })
}

pub fn itemconfig(w: W, id: i64, opts: Vec<(&str, Val)>) {
    with(|a| {
        if let Some(it) = a.widgets[w].items.iter_mut().find(|it| it.id == id) {
            it.opts.extend(opts_map(opts));
        }
    })
}

// ---------------------------------------------------------------- events and time

/// `w.bind("<Return>", f)` (f gets the event).
pub fn bind(w: W, ev: &str, cb: EvCb) {
    with(|a| a.widgets[w].binds.push((ev.to_string(), cb)))
}

/// `root.after(ms, f)`; returns the id `after_cancel` takes.
pub fn after(ms: i64, cb: Cb) -> i64 {
    with(|a| {
        a.seq += 1;
        let t = Timer { due: a.now_ms + ms.max(0) as u64, seq: a.seq, id: a.seq as i64, cb };
        a.timers.push(t);
        a.seq as i64
    })
}

pub fn after_cancel(id: i64) {
    with(|a| a.timers.retain(|t| t.id != id))
}

/// Runs the timers due by `now_ms`, earliest first (a timer added by a timer runs too, when due).
pub(crate) fn run_timers(until: u64) {
    loop {
        let next = with(|a| {
            let k = a.timers.iter().enumerate().filter(|(_, t)| t.due <= until).min_by_key(|(_, t)| (t.due, t.seq)).map(|(k, _)| k)?;
            let t = a.timers.remove(k);
            a.now_ms = a.now_ms.max(t.due);
            Some(t.cb)
        });
        match next {
            Some(cb) => cb(),
            None => break,
        }
    }
    with(|a| a.now_ms = a.now_ms.max(until));
}

/// Runs the widget's command (a Button click, a Checkbutton toggle) the way a click does.
pub fn invoke(w: W) {
    let (kind, cmd, var, onv, offv, val) = with(|a| {
        let x = &a.widgets[w];
        let var = match x.opts.get("variable") { Some(Val::Var(v)) => Some(*v), _ => None };
        (x.kind, x.opts.get("command").cloned(), var, x.opts.get("onvalue").cloned(), x.opts.get("offvalue").cloned(), x.opts.get("value").cloned())
    });
    if cget(w, "state").text() == "disabled" {
        return;
    }
    match kind {
        Kind::Checkbutton => {
            let now = !is_checked(w);
            match var {
                Some(v) => var_set(v, if now { onv.unwrap_or(Val::Int(1)) } else { offv.unwrap_or(Val::Int(0)) }),
                None => with(|a| a.widgets[w].checked = now),
            }
        }
        Kind::Radiobutton => match var {
            Some(v) => var_set(v, val.unwrap_or(Val::None)),
            None => with(|a| a.widgets[w].checked = true),
        },
        _ => {}
    }
    if let Some(Val::Cb(f)) = cmd {
        f();
    }
}

/// Runs the callbacks bound to `ev` on `w` (and `<Key>` for any key).
pub(crate) fn fire(w: W, ev: &str, e: &Event) {
    let cbs: Vec<EvCb> = with(|a| a.widgets[w].binds.iter().filter(|(k, _)| k == ev || (k == "<Key>" && ev.starts_with("<Key"))).map(|(_, c)| c.clone()).collect());
    for c in cbs {
        c(e);
    }
}

// ---------------------------------------------------------------- dialogs

/// `messagebox.showinfo(title, msg)` and friends. Returns what tkinter returns: `"ok"` for show*,
/// True/False for ask*. Headless, the dialog is printed and the answer is the next `answer` line given
/// (else yes/ok).
pub fn messagebox(kind: &str, title: &str, msg: &str) -> Val {
    if headless::active() {
        return headless::dialog(kind, title, msg);
    }
    #[cfg(feature = "window")]
    {
        return window::dialog(kind, title, msg);
    }
    #[allow(unreachable_code)]
    headless::dialog(kind, title, msg)
}

/// `root.mainloop()`: draws the window and runs callbacks until it closes (or, headless, until stdin ends).
pub fn mainloop() {
    tk();
    if headless::active() {
        headless::run();
        return;
    }
    #[cfg(feature = "window")]
    {
        window::run();
        return;
    }
    #[allow(unreachable_code)]
    headless::run();
}

/// Read access for the front ends.
pub(crate) fn snapshot<R>(f: impl FnOnce(&[Widget]) -> R) -> R {
    APP.with(|a| f(&a.borrow().widgets))
}

pub(crate) fn quitting() -> bool {
    with(|a| a.quit)
}

pub(crate) fn now() -> u64 {
    with(|a| a.now_ms)
}

pub(crate) fn push_answer(s: String) {
    with(|a| a.answers.push(s))
}

pub(crate) fn take_answer() -> Option<String> {
    with(|a| if a.answers.is_empty() { None } else { Some(a.answers.remove(0)) })
}
