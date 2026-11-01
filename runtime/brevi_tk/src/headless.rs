//! Headless front end: no window. `mainloop` reads actions from stdin, one per line, and `show` prints the
//! window as text. This is how tests (and AI models, which cannot see a screen) drive a GUI program.
//!
//!   show                 print every visible widget, indented under its parent
//!   click TEXT           click the button, checkbutton or radiobutton showing TEXT
//!   type N TEXT          replace the text of the Nth text box (Entry, Text, Spinbox, Combobox; from 1)
//!   key N KEY            press KEY (Return, Escape, a, ...) in the Nth text box (0: the window)
//!   select N I           select line I (from 0) of the Nth list box
//!   slide N X            move the Nth slider to X
//!   tap N X Y            click the Nth canvas at X, Y
//!   wait MS              let MS milliseconds pass (runs `after` timers)
//!   answer yes|no|TEXT   the answer to the next dialog (default: yes / ok)
//!   close                close the window
//! Lines starting with `#` and blank lines are skipped. An action that matches nothing prints `? LINE`.
use crate::*;
use std::io::BufRead;

pub(crate) fn active() -> bool {
    if !cfg!(feature = "window") {
        return true;
    }
    match std::env::var("BREVI_GUI") {
        Ok(v) => v == "headless",
        // No display on Linux: run headless rather than fail.
        Err(_) => cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none(),
    }
}

pub(crate) fn dialog(kind: &str, title: &str, msg: &str) -> Val {
    let ans = take_answer();
    println!("[{kind}] {title}: {msg}");
    match kind {
        "askyesno" | "askokcancel" | "askretrycancel" => Val::Bool(ans.map(|a| a == "yes" || a == "ok" || a == "retry").unwrap_or(true)),
        "askyesnocancel" => match ans.as_deref() {
            Some("no") => Val::Bool(false),
            Some("cancel") => Val::None,
            _ => Val::Bool(true),
        },
        "askquestion" => Val::Str(ans.unwrap_or_else(|| "yes".into())),
        "askstring" => ans.map(Val::Str).unwrap_or(Val::None),
        "askinteger" => ans.and_then(|a| a.parse::<i64>().ok()).map(Val::Int).unwrap_or(Val::None),
        "askfloat" => ans.and_then(|a| a.parse::<f64>().ok()).map(Val::Float).unwrap_or(Val::None),
        _ => Val::Str("ok".into()),
    }
}

fn num(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 { format!("{}", x as i64) } else { format!("{x}") }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

// Reads for `show`, which holds the whole table (so it cannot call `var_get`, which borrows it again).
fn vtext(a: &App, v: &Val) -> String {
    match v {
        Val::Var(x) => a.vars[*x].val.text(),
        v => v.text(),
    }
}

fn opt(a: &App, w: W, k: &str) -> Option<Val> {
    match a.widgets[w].opts.get(k) {
        Some(Val::Var(x)) => Some(a.vars[*x].val.clone()),
        v => v.cloned(),
    }
}

/// The text a widget shows: its textvariable, else its `text` option.
fn label_text(a: &App, w: W) -> String {
    opt(a, w, "textvariable").or_else(|| opt(a, w, "text")).map(|v| v.text()).unwrap_or_default()
}

fn box_text(a: &App, w: W) -> String {
    opt(a, w, "textvariable").map(|v| v.text()).unwrap_or_else(|| a.widgets[w].text.clone())
}

fn checked(a: &App, w: W) -> bool {
    let x = &a.widgets[w];
    match (x.opts.get("variable"), x.kind) {
        (Some(Val::Var(v)), Kind::Radiobutton) => a.vars[*v].val.text() == x.opts.get("value").map(|v| vtext(a, v)).unwrap_or_default(),
        (Some(Val::Var(v)), _) => match x.opts.get("onvalue") {
            Some(o) => a.vars[*v].val.text() == vtext(a, o),
            None => a.vars[*v].val.truthy(),
        },
        _ => x.checked,
    }
}

fn scale_value(a: &App, w: W) -> f64 {
    opt(a, w, "variable").or_else(|| opt(a, w, "value")).or_else(|| opt(a, w, "from")).map(|v| v.float()).unwrap_or(0.0)
}

/// Visible children in display order (grid cells by row then column; others as created).
pub(crate) fn shown(ws: &[Widget], p: W) -> Vec<W> {
    let mut cs: Vec<W> = ws[p].children.iter().copied().filter(|&c| ws[c].alive && !matches!(ws[c].place, Place::None) && ws[c].kind != Kind::Toplevel).collect();
    cs.sort_by_key(|&c| match ws[c].place {
        Place::Grid { row, column, .. } => (row, column),
        _ => (0, 0),
    });
    cs
}

fn describe(a: &App, w: W, depth: usize, out: &mut Vec<String>) {
    let x = &a.widgets[w];
    let pad = "  ".repeat(depth);
    let disabled = if x.opts.get("state").map(|v| v.text()).as_deref() == Some("disabled") { " (disabled)" } else { "" };
    let line = match x.kind {
        Kind::Tk | Kind::Toplevel => format!("window {}", quote(&x.title)),
        Kind::Frame => "Frame".to_string(),
        Kind::LabelFrame => format!("LabelFrame {}", quote(&label_text(a, w))),
        Kind::Label | Kind::Button => format!("{:?} {}", x.kind, quote(&label_text(a, w))),
        Kind::Entry | Kind::Spinbox | Kind::Text => format!("{:?} {}", x.kind, quote(&box_text(a, w))),
        Kind::Combobox => format!("Combobox {} of [{}]", quote(&box_text(a, w)), x.lines.join(", ")),
        Kind::Checkbutton => format!("Checkbutton {} [{}]", quote(&label_text(a, w)), if checked(a, w) { "x" } else { " " }),
        Kind::Radiobutton => format!("Radiobutton {} ({})", quote(&label_text(a, w)), if checked(a, w) { "o" } else { " " }),
        Kind::Listbox => {
            let ls: Vec<String> = x.lines.iter().enumerate().map(|(i, l)| if x.selection.contains(&i) { format!("{l}*") } else { l.clone() }).collect();
            format!("Listbox [{}]", ls.join(" | "))
        }
        Kind::Scale => format!("Scale {}", num(scale_value(a, w))),
        Kind::Progressbar => format!("Progressbar {}", opt(a, w, "variable").or_else(|| opt(a, w, "value")).map(|v| num(v.float())).unwrap_or_else(|| "0".into())),
        Kind::Canvas => format!("Canvas {}x{}", x.opts.get("width").map(|v| v.int()).unwrap_or(0), x.opts.get("height").map(|v| v.int()).unwrap_or(0)),
        Kind::Scrollbar => return,
        Kind::Separator => "Separator".to_string(),
    };
    out.push(format!("{pad}{line}{disabled}"));
    if x.kind == Kind::Canvas {
        for it in &x.items {
            let cs: Vec<String> = it.coords.iter().map(|c| num(*c)).collect();
            let os: Vec<String> = it.opts.iter().filter(|(k, _)| k.as_str() != "tags").map(|(k, v)| format!("{k}={}", vtext(a, v))).collect();
            out.push(format!("{pad}  {} {}{}{}", it.kind, cs.join(" "), if os.is_empty() { "" } else { " " }, os.join(" ")));
        }
    }
    for c in shown(&a.widgets, w) {
        describe(a, c, depth + 1, out);
    }
}

/// Every open window as text, one widget a line.
pub fn show() -> String {
    APP.with(|a| {
        let a = a.borrow();
        let mut out = vec![];
        let mut roots = vec![0];
        roots.extend(a.widgets.iter().enumerate().filter(|(_, w)| w.kind == Kind::Toplevel && w.alive).map(|(i, _)| i));
        for r in roots {
            if a.widgets[r].alive {
                describe(&a, r, 0, &mut out);
            }
        }
        out.join("\n")
    })
}

/// The text a clickable widget shows (for `click`).
fn click_text(w: W) -> String {
    APP.with(|a| label_text(&a.borrow(), w))
}

fn nth(kinds: &[Kind], n: usize) -> Option<W> {
    fn walk(ws: &[Widget], w: W, order: &mut Vec<W>) {
        order.push(w);
        for c in shown(ws, w) {
            walk(ws, c, order);
        }
    }
    snapshot(|ws| {
        let mut order = vec![];
        let mut roots = vec![0];
        roots.extend(ws.iter().enumerate().filter(|(_, w)| w.kind == Kind::Toplevel && w.alive).map(|(i, _)| i));
        for r in roots {
            walk(ws, r, &mut order);
        }
        order.into_iter().filter(|&w| kinds.contains(&ws[w].kind)).nth(n.checked_sub(1)?)
    })
}

fn find_clickable(text: &str) -> Option<W> {
    let cands: Vec<W> = snapshot(|ws| ws.iter().enumerate().filter(|(_, w)| w.alive && matches!(w.kind, Kind::Button | Kind::Checkbutton | Kind::Radiobutton) && !matches!(w.place, Place::None)).map(|(i, _)| i).collect());
    cands.into_iter().find(|&w| click_text(w) == text)
}

const TEXTS: &[Kind] = &[Kind::Entry, Kind::Text, Kind::Spinbox, Kind::Combobox];

fn key_event(w: W, key: &str) {
    let e = Event { widget: w, keysym: key.to_string(), char: if key.chars().count() == 1 { key.to_string() } else { String::new() }, x: 0, y: 0 };
    let names = [format!("<{key}>"), format!("<KeyPress-{key}>"), format!("<Key-{key}>"), if key.chars().count() == 1 { key.to_string() } else { String::new() }];
    for n in names.iter().filter(|n| !n.is_empty()) {
        fire(w, n, &e);
        if w != 0 {
            fire(0, n, &e);
        }
    }
    fire(w, "<Key>", &e);
}

pub(crate) fn run() {
    let stdin = std::io::stdin();
    run_timers(now());
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if quitting() {
            break;
        }
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let (cmd, rest) = l.split_once(' ').unwrap_or((l, ""));
        let mut args = rest.splitn(2, ' ');
        let a1 = args.next().unwrap_or("");
        let a2 = args.next().unwrap_or("");
        let ok = match cmd {
            "show" => {
                println!("{}", show());
                true
            }
            "click" => match find_clickable(rest) {
                Some(w) => {
                    invoke(w);
                    true
                }
                None => false,
            },
            "type" => match a1.parse().ok().and_then(|n| nth(TEXTS, n)) {
                Some(w) => {
                    set_text(w, a2);
                    true
                }
                None => false,
            },
            "key" => {
                let w = if a1 == "0" { Some(0) } else { a1.parse().ok().and_then(|n| nth(TEXTS, n)) };
                match w {
                    Some(w) => {
                        key_event(w, a2);
                        true
                    }
                    None => false,
                }
            }
            "select" => match (a1.parse().ok().and_then(|n| nth(&[Kind::Listbox], n)), a2.parse::<i64>()) {
                (Some(w), Ok(i)) if i >= 0 && i < size(w) => {
                    selection_clear(w);
                    selection_set(w, i);
                    fire(w, "<<ListboxSelect>>", &Event { widget: w, ..Default::default() });
                    true
                }
                _ => false,
            },
            "slide" => match (a1.parse().ok().and_then(|n| nth(&[Kind::Scale], n)), a2.parse::<f64>()) {
                (Some(w), Ok(x)) => {
                    scale_set(w, x);
                    if let Val::Cb(f) = cget(w, "command") {
                        f();
                    }
                    true
                }
                _ => false,
            },
            "tap" => {
                let mut xy = a2.split(' ').map(|s| s.parse::<i64>());
                match (a1.parse().ok().and_then(|n| nth(&[Kind::Canvas], n)), xy.next(), xy.next()) {
                    (Some(w), Some(Ok(x)), Some(Ok(y))) => {
                        let e = Event { widget: w, x, y, ..Default::default() };
                        for n in ["<Button-1>", "<Button>", "<1>", "<ButtonPress-1>"] {
                            fire(w, n, &e);
                        }
                        true
                    }
                    _ => false,
                }
            }
            "wait" => match a1.parse::<u64>() {
                Ok(ms) => {
                    run_timers(now() + ms);
                    true
                }
                Err(_) => false,
            },
            "answer" => {
                push_answer(rest.to_string());
                true
            }
            "close" => {
                destroy(0);
                true
            }
            _ => false,
        };
        if !ok {
            println!("? {l}");
        }
        run_timers(now());
    }
}
