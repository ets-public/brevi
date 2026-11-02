// Glue between a compiled tkinter program and brevi_tk (pasted in by gui::runtime, not compiled here).
#[allow(dead_code, unused)]
mod tkx_glue {
    use super::*;
    use super::brevi_tk as tk;
    use std::cell::RefCell;
    use std::rc::Rc;

    const VAR_BASE: i64 = 1 << 40;

    thread_local! {
        // Callback id (from 1) -> (callback k, captured entry i).
        static CBS: RefCell<Vec<(i64, i64)>> = const { RefCell::new(Vec::new()) };
    }

    pub fn cbreg(k: i64, i: i64) -> i64 {
        CBS.with(|c| {
            c.borrow_mut().push((k, i));
            c.borrow().len() as i64
        })
    }

    pub trait Done {
        fn done(self);
    }
    impl Done for () {
        fn done(self) {}
    }
    impl<E: std::fmt::Debug> Done for Result<(), E> {
        fn done(self) {
            if let Err(e) = self {
                eprintln!("{{\"e\":\"runtime\",\"msg\":{:?}}}", format!("{e:?}"));
                std::process::exit(1);
            }
        }
    }

    fn run(id: i64, e: &tk::Event) {
        let Some((k, i)) = CBS.with(|c| c.borrow().get((id - 1).max(0) as usize).copied()) else { return };
        if id <= 0 {
            return;
        }
        let ev = TkEvent { widget: e.widget as i64, keysym: e.keysym.clone(), char_: e.char.clone(), x: e.x, y: e.y, delta: 0, num: 1 };
        tkx_dispatch(k, i, ev).done();
    }

    fn cb(id: i64) -> tk::Cb {
        Rc::new(move || run(id, &tk::Event::default()))
    }

    fn evcb(id: i64) -> tk::EvCb {
        Rc::new(move |e: &tk::Event| run(id, e))
    }

    pub fn val(j: &Json) -> tk::Val {
        match j {
            Json::Null => tk::Val::None,
            Json::Bool { v } => tk::Val::Bool(*v),
            Json::Num { v } => {
                if v.fract() == 0.0 && v.abs() < 9e15 {
                    tk::Val::Int(*v as i64)
                } else {
                    tk::Val::Float(*v)
                }
            }
            Json::Str { v } => tk::Val::Str(v.clone()),
            Json::Arr { v } => tk::Val::List(v.iter().map(val).collect()),
            Json::Obj { .. } => tk::Val::None,
        }
    }

    fn num(j: &Json) -> i64 {
        val(j).int()
    }

    // Keyword arguments: `[name, value, name, value, ...]`.
    fn opts(kw: &[Json]) -> Vec<(String, tk::Val)> {
        kw.chunks(2)
            .filter_map(|p| {
                let Json::Str { v: k } = &p[0] else { return None };
                let k = k.trim_end_matches('_').to_string();
                let v = match k.as_str() {
                    "command" | "postcommand" | "validatecommand" | "invalidcommand" | "xscrollcommand" | "yscrollcommand" => match num(&p[1]) {
                        0 => tk::Val::None,
                        id => tk::Val::Cb(cb(id)),
                    },
                    "variable" | "textvariable" | "listvariable" => match num(&p[1]) {
                        h if h >= VAR_BASE => tk::Val::Var((h - VAR_BASE) as usize),
                        _ => tk::Val::None,
                    },
                    _ => val(&p[1]),
                };
                Some((k, v))
            })
            .collect()
    }

    fn refs(o: &[(String, tk::Val)]) -> Vec<(&str, tk::Val)> {
        o.iter().map(|(k, v)| (k.as_str(), v.clone())).collect()
    }

    pub fn list(v: tk::Val) -> Vec<tk::Val> {
        match v {
            tk::Val::List(xs) => xs,
            tk::Val::None => vec![],
            x => vec![x],
        }
    }

    pub fn root() -> i64 {
        tk::tk() as i64
    }

    pub fn new(kind: &str, pos: Vec<Json>, kw: Vec<Json>) -> i64 {
        let parent = pos.first().map(|p| num(p)).filter(|p| *p >= 0 && *p < VAR_BASE).unwrap_or(0) as usize;
        let o = opts(&kw);
        tk::widget(kind, parent, refs(&o)) as i64
    }

    pub fn var(kind: &str, pos: Vec<Json>, kw: Vec<Json>) -> i64 {
        let o = opts(&kw);
        // StringVar(master, value) or StringVar(value=...)
        let v = o.iter().find(|(k, _)| k == "value").map(|(_, v)| v.clone()).or_else(|| pos.get(1).map(val));
        tk::var(kind, v) as i64 + VAR_BASE
    }

    fn f64s(xs: &[Json]) -> Vec<f64> {
        let mut out = vec![];
        for x in xs {
            for v in list(val(x)) {
                match v {
                    tk::Val::List(ys) => out.extend(ys.iter().map(|y| y.float())),
                    v => out.push(v.float()),
                }
            }
        }
        out
    }

    // Canvas items by id or tag.
    fn items(w: usize, tag: &tk::Val) -> Vec<i64> {
        tk::APP.with(|a| {
            let a = a.borrow();
            let t = tag.text();
            a.widgets[w].items.iter().filter(|it| t == "all" || matches!(tag, tk::Val::Int(i) if *i == it.id) || it.opts.get("tags").map(|x| list(x.clone()).iter().any(|y| y.text() == t)).unwrap_or(false)).map(|it| it.id).collect()
        })
    }

    pub fn call(w: i64, m: &str, pos: Vec<Json>, kw: Vec<Json>) -> tk::Val {
        use tk::Val;
        let p = |k: usize| pos.get(k).map(val).unwrap_or(Val::None);
        let o = opts(&kw);
        // Dialogs: messagebox.showinfo(title, message), simpledialog.askstring(title, prompt).
        if let Some((sub, f)) = m.split_once('.') {
            let get = |k: usize, name: &str| o.iter().find(|(n, _)| n == name).map(|(_, v)| v.text()).unwrap_or_else(|| p(k).text());
            return match sub {
                "messagebox" | "simpledialog" | "filedialog" => tk::messagebox(f, &get(0, "title"), &get(1, if sub == "messagebox" { "message" } else { "prompt" })),
                _ => Val::None,
            };
        }
        if w >= VAR_BASE {
            let v = (w - VAR_BASE) as usize;
            return match m {
                "get" => tk::var_get(v),
                "set" => {
                    tk::var_set(v, p(0));
                    Val::None
                }
                "trace_add" | "trace" | "trace_variable" => {
                    let id = num(&pos[1]);
                    tk::var_trace(v, cb(id));
                    Val::None
                }
                _ => Val::None,
            };
        }
        if w < 0 {
            return Val::None;
        }
        let w = w as usize;
        let kind = tk::APP.with(|a| a.borrow().widgets.get(w).map(|x| x.kind));
        let Some(kind) = kind else { return Val::None };
        match m {
            "pack" => tk::pack(w, refs(&o)),
            "grid" => tk::grid(w, refs(&o)),
            "place" => tk::place(w, refs(&o)),
            "pack_forget" | "grid_forget" | "place_forget" | "grid_remove" => tk::pack_forget(w),
            "destroy" => tk::destroy(w),
            "quit" => tk::quit(),
            "mainloop" => tk::mainloop(),
            "config" | "configure" => {
                if kind == tk::Kind::Tk || kind == tk::Kind::Toplevel {
                    if let Some((_, t)) = o.iter().find(|(k, _)| k == "title") {
                        tk::title(w, &t.text());
                    }
                }
                tk::config(w, refs(&o))
            }
            "cget" => return tk::cget(w, &p(0).text()),
            "title" => {
                if pos.is_empty() {
                    return Val::Str(tk::APP.with(|a| a.borrow().widgets[w].title.clone()));
                }
                tk::title(w, &p(0).text())
            }
            "geometry" => tk::geometry(w, &p(0).text()),
            "get" => {
                return match kind {
                    tk::Kind::Listbox if pos.len() >= 2 => {
                        let n = tk::size(w);
                        let i = p(0).int();
                        let j = match p(1) { Val::Str(s) if s.starts_with("end") => n - 1, v => v.int() };
                        Val::List((i..=j).map(|k| Val::Str(tk::line(w, k))).collect())
                    }
                    tk::Kind::Listbox => Val::Str(tk::line(w, match p(0) { Val::Str(s) if s.starts_with("end") => tk::size(w) - 1, v => v.int() })),
                    tk::Kind::Text => Val::Str(tk::text_get(w, p(0), pos.get(1).map(val))),
                    tk::Kind::Scale => {
                        let x = tk::scale_get(w);
                        if x.fract() == 0.0 { Val::Int(x as i64) } else { Val::Float(x) }
                    }
                    _ => Val::Str(tk::get(w)),
                };
            }
            "set" => match kind {
                tk::Kind::Scale => tk::scale_set(w, p(0).float()),
                tk::Kind::Combobox | tk::Kind::Spinbox | tk::Kind::Entry => tk::set_text(w, &p(0).text()),
                _ => {}
            },
            "insert" => {
                if kind == tk::Kind::Listbox {
                    let mut at = p(0);
                    for k in 1..pos.len() {
                        tk::insert(w, at.clone(), p(k));
                        if let Val::Int(i) = at {
                            at = Val::Int(i + 1);
                        }
                    }
                } else {
                    tk::insert(w, p(0), p(1));
                }
            }
            "delete" => {
                if kind == tk::Kind::Canvas {
                    for k in 0..pos.len() {
                        let t = p(k);
                        for id in items(w, &t) {
                            tk::delete(w, Val::Int(id), None);
                        }
                    }
                } else {
                    tk::delete(w, p(0), pos.get(1).map(val));
                }
            }
            "curselection" => return Val::List(tk::curselection(w).into_iter().map(Val::Int).collect()),
            "selection_set" | "select_set" => {
                let i = p(0).int();
                let j = pos.get(1).map(|x| val(x).int()).unwrap_or(i);
                for k in i..=j {
                    tk::selection_set(w, k);
                }
            }
            "selection_clear" | "select_clear" => tk::selection_clear(w),
            "size" => return Val::Int(tk::size(w)),
            "index" => {
                return match p(0) {
                    Val::Str(s) if s.starts_with("end") => Val::Int(tk::size(w)),
                    v => Val::Int(v.int()),
                }
            }
            "current" => {
                let lines = tk::APP.with(|a| a.borrow().widgets[w].lines.clone());
                if pos.is_empty() {
                    let t = tk::get(w);
                    return Val::Int(lines.iter().position(|l| *l == t).map_or(-1, |i| i as i64));
                }
                let i = p(0).int().max(0) as usize;
                tk::set_text(w, lines.get(i).map(|s| s.as_str()).unwrap_or(""));
            }
            "invoke" | "toggle" => tk::invoke(w),
            "select" | "deselect" => {
                let on = m == "select";
                let (var, onv, offv, value) = tk::APP.with(|a| {
                    let x = &a.borrow().widgets[w];
                    let var = match x.opts.get("variable") { Some(Val::Var(v)) => Some(*v), _ => None };
                    (var, x.opts.get("onvalue").cloned(), x.opts.get("offvalue").cloned(), x.opts.get("value").cloned())
                });
                match var {
                    Some(v) if kind == tk::Kind::Radiobutton => tk::var_set(v, value.unwrap_or(Val::None)),
                    Some(v) => tk::var_set(v, if on { onv.unwrap_or(Val::Int(1)) } else { offv.unwrap_or(Val::Int(0)) }),
                    None => tk::APP.with(|a| a.borrow_mut().widgets[w].checked = on),
                }
            }
            "bind" | "bind_all" => {
                let id = pos.get(1).map(num).unwrap_or(0);
                if id > 0 {
                    tk::bind(if m == "bind_all" { 0 } else { w }, &p(0).text(), evcb(id));
                }
            }
            "after" => {
                let id = pos.get(1).map(num).unwrap_or(0);
                if id > 0 {
                    return Val::Int(tk::after(p(0).int(), cb(id)));
                }
            }
            "after_idle" => return Val::Int(tk::after(0, cb(pos.first().map(num).unwrap_or(0)))),
            "after_cancel" => tk::after_cancel(p(0).int()),
            c if c.starts_with("create_") => {
                let kind = &c[7..];
                return Val::Int(tk::create(w, kind, f64s(&pos), refs(&o)));
            }
            "coords" => {
                let ids = items(w, &p(0));
                let xs = f64s(&pos[1.min(pos.len())..]);
                let mut out = vec![];
                for id in ids.into_iter().take(1) {
                    out = tk::coords(w, id, xs.clone());
                }
                return Val::List(out.into_iter().map(Val::Float).collect());
            }
            "move" => {
                for id in items(w, &p(0)) {
                    tk::move_item(w, id, p(1).float(), p(2).float());
                }
            }
            "itemconfig" | "itemconfigure" => {
                for id in items(w, &p(0)) {
                    tk::itemconfig(w, id, refs(&o));
                }
            }
            "itemcget" => {
                let id = items(w, &p(0)).first().copied().unwrap_or(-1);
                let k = p(1).text();
                return tk::APP.with(|a| a.borrow().widgets[w].items.iter().find(|it| it.id == id).and_then(|it| it.opts.get(&k).cloned()).unwrap_or(Val::None));
            }
            "find_all" => return Val::List(items(w, &Val::Str("all".into())).into_iter().map(Val::Int).collect()),
            "find_withtag" => return Val::List(items(w, &p(0)).into_iter().map(Val::Int).collect()),
            "winfo_exists" => return Val::Bool(tk::APP.with(|a| a.borrow().widgets[w].alive)),
            "winfo_width" | "winfo_reqwidth" => return Val::Int(tk::cget(w, "width").int()),
            "winfo_height" | "winfo_reqheight" => return Val::Int(tk::cget(w, "height").int()),
            _ => {}
        }
        Val::None
    }
}
