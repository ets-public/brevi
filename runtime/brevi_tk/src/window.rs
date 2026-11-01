//! Window front end: draws the widget tree with egui every frame and turns clicks and typing into the same
//! calls the headless front end makes (`invoke`, `set_text`, `fire`, ...).
use crate::*;
use eframe::egui;
use std::time::Instant;

/// What the user did during a frame; applied after drawing, since callbacks change the widget table.
enum Act {
    Click(W),
    SetText(W, String),
    Select(W, usize),
    Slide(W, f64),
    Choose(W, String),
    Key(W, String),
    Tap(W, i64, i64),
}

fn color(s: &str) -> Option<egui::Color32> {
    let s = s.trim().to_lowercase();
    if let Some(h) = s.strip_prefix('#') {
        let v = u32::from_str_radix(h, 16).ok()?;
        return Some(match h.len() {
            6 => egui::Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8),
            3 => egui::Color32::from_rgb(((v >> 8) & 15) as u8 * 17, ((v >> 4) & 15) as u8 * 17, (v & 15) as u8 * 17),
            _ => return None,
        });
    }
    Some(match s.as_str() {
        "black" => egui::Color32::BLACK,
        "white" => egui::Color32::WHITE,
        "red" => egui::Color32::from_rgb(220, 40, 40),
        "green" => egui::Color32::from_rgb(40, 160, 60),
        "blue" => egui::Color32::from_rgb(40, 80, 220),
        "yellow" => egui::Color32::from_rgb(240, 210, 40),
        "orange" => egui::Color32::from_rgb(240, 140, 30),
        "purple" => egui::Color32::from_rgb(140, 60, 180),
        "gray" | "grey" => egui::Color32::GRAY,
        "lightgray" | "lightgrey" => egui::Color32::LIGHT_GRAY,
        "darkgray" | "darkgrey" => egui::Color32::DARK_GRAY,
        "cyan" => egui::Color32::from_rgb(40, 200, 220),
        "magenta" | "pink" => egui::Color32::from_rgb(220, 60, 180),
        "brown" => egui::Color32::from_rgb(140, 80, 40),
        _ => return None,
    })
}

struct Ui {
    started: Instant,
    /// Text being edited, per text box, so typing is not lost between frames.
    edits: std::collections::HashMap<W, String>,
    dialogs: Vec<(String, String)>,
}

thread_local! {
    static DIALOGS: std::cell::RefCell<Vec<(String, String)>> = const { std::cell::RefCell::new(vec![]) };
}

pub(crate) fn dialog(kind: &str, title: &str, msg: &str) -> Val {
    // egui cannot block inside a callback: the message is shown in a small window on the next frame,
    // and questions are answered yes.
    DIALOGS.with(|d| d.borrow_mut().push((title.to_string(), msg.to_string())));
    match kind {
        k if k.starts_with("ask") && k != "askquestion" && k != "askstring" && k != "askinteger" && k != "askfloat" => Val::Bool(true),
        "askquestion" => Val::Str("yes".into()),
        k if k.starts_with("ask") => Val::None,
        _ => Val::Str("ok".into()),
    }
}

fn text_of(w: W) -> String {
    APP.with(|a| {
        let a = a.borrow();
        match a.widgets[w].opts.get("textvariable") {
            Some(Val::Var(v)) => a.vars[*v].val.text(),
            _ => match a.widgets[w].kind {
                Kind::Entry | Kind::Text | Kind::Spinbox | Kind::Combobox => a.widgets[w].text.clone(),
                _ => a.widgets[w].opts.get("text").map(|v| v.text()).unwrap_or_default(),
            },
        }
    })
}

impl Ui {
    fn draw(&mut self, ui: &mut egui::Ui, w: W, acts: &mut Vec<Act>) {
        let (kind, kids, opts) = snapshot(|ws| (ws[w].kind, crate::headless::shown(ws, w), ws[w].opts.clone()));
        let fg = opts.get("fg").or(opts.get("foreground")).and_then(|v| color(&v.text()));
        let disabled = opts.get("state").map(|v| v.text()) == Some("disabled".into());
        match kind {
            Kind::Tk | Kind::Toplevel | Kind::Frame | Kind::LabelFrame => {
                let body = |s: &mut Self, ui: &mut egui::Ui, acts: &mut Vec<Act>| s.container(ui, &kids, acts);
                if kind == Kind::LabelFrame {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(text_of(w)).strong());
                        body(self, ui, acts)
                    });
                } else {
                    body(self, ui, acts);
                }
            }
            Kind::Label => {
                let mut t = egui::RichText::new(text_of(w));
                if let Some(c) = fg {
                    t = t.color(c);
                }
                ui.label(t);
            }
            Kind::Button => {
                if ui.add_enabled(!disabled, egui::Button::new(text_of(w))).clicked() {
                    acts.push(Act::Click(w));
                }
            }
            Kind::Entry | Kind::Spinbox | Kind::Text => {
                let cur = text_of(w);
                let buf = self.edits.entry(w).or_insert_with(|| cur.clone());
                // A change made by the program wins over the box's copy.
                if *buf != cur && !ui.memory(|m| m.has_focus(egui::Id::new(("box", w)))) {
                    *buf = cur.clone();
                }
                let width = opts.get("width").map(|v| v.int() as f32 * 8.0).unwrap_or(160.0);
                let te = if kind == Kind::Text { egui::TextEdit::multiline(buf) } else { egui::TextEdit::singleline(buf) };
                let te = te.id(egui::Id::new(("box", w))).desired_width(width).password(opts.get("show").map(|v| !v.text().is_empty()).unwrap_or(false));
                let r = ui.add_enabled(!disabled, te);
                if r.changed() {
                    acts.push(Act::SetText(w, buf.clone()));
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    acts.push(Act::Key(w, "Return".into()));
                }
            }
            Kind::Checkbutton => {
                let mut on = is_checked(w);
                if ui.add_enabled(!disabled, egui::Checkbox::new(&mut on, text_of(w))).clicked() {
                    acts.push(Act::Click(w));
                }
            }
            Kind::Radiobutton => {
                if ui.add_enabled(!disabled, egui::RadioButton::new(is_checked(w), text_of(w))).clicked() {
                    acts.push(Act::Click(w));
                }
            }
            Kind::Listbox => {
                let (lines, sel) = snapshot(|ws| (ws[w].lines.clone(), ws[w].selection.clone()));
                let h = opts.get("height").map(|v| v.int() as f32 * 18.0).unwrap_or(160.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt(("lb", w)).max_height(h).show(ui, |ui| {
                        for (i, l) in lines.iter().enumerate() {
                            if ui.selectable_label(sel.contains(&i), l).clicked() {
                                acts.push(Act::Select(w, i));
                            }
                        }
                    });
                });
            }
            Kind::Combobox => {
                let (lines, cur) = (snapshot(|ws| ws[w].lines.clone()), text_of(w));
                egui::ComboBox::from_id_salt(("cb", w)).selected_text(cur.clone()).show_ui(ui, |ui| {
                    for l in &lines {
                        if ui.selectable_label(*l == cur, l).clicked() {
                            acts.push(Act::Choose(w, l.clone()));
                        }
                    }
                });
            }
            Kind::Scale => {
                let lo = opts.get("from").map(|v| v.float()).unwrap_or(0.0);
                let hi = opts.get("to").map(|v| v.float()).unwrap_or(100.0);
                let mut x = scale_get(w);
                let mut s = egui::Slider::new(&mut x, lo..=hi);
                if let Some(l) = opts.get("label") {
                    s = s.text(l.text());
                }
                if ui.add_enabled(!disabled, s).changed() {
                    acts.push(Act::Slide(w, x));
                }
            }
            Kind::Progressbar => {
                let max = opts.get("maximum").map(|v| v.float()).unwrap_or(100.0);
                let v = opts.get("value").or(opts.get("variable")).map(|v| v.float()).unwrap_or(0.0);
                ui.add(egui::ProgressBar::new((v / max.max(1e-9)) as f32).desired_width(200.0));
            }
            Kind::Canvas => self.canvas(ui, w, &opts, acts),
            Kind::Separator => {
                ui.separator();
            }
            Kind::Scrollbar => {}
        }
    }

    /// pack: `side` top/bottom stack, left/right runs sit in a row; grid: an egui grid by row and column.
    fn container(&mut self, ui: &mut egui::Ui, kids: &[W], acts: &mut Vec<Act>) {
        let places: Vec<(W, Place)> = snapshot(|ws| kids.iter().map(|&k| (k, ws[k].place.clone())).collect());
        if places.iter().any(|(_, p)| matches!(p, Place::Grid { .. })) {
            let mut rows: std::collections::BTreeMap<i64, Vec<(i64, W)>> = Default::default();
            for (k, p) in &places {
                if let Place::Grid { row, column, .. } = p {
                    rows.entry(*row).or_default().push((*column, *k));
                }
            }
            egui::Grid::new(("grid", kids.first().copied().unwrap_or(0))).spacing([8.0, 6.0]).show(ui, |ui| {
                for (_, mut cells) in rows {
                    cells.sort();
                    let mut col = 0;
                    for (c, k) in cells {
                        while col < c {
                            ui.label("");
                            col += 1;
                        }
                        self.draw(ui, k, acts);
                        col += 1;
                    }
                    ui.end_row();
                }
            });
            return;
        }
        let mut i = 0;
        while i < places.len() {
            let horiz = |p: &Place| matches!(p, Place::Pack { side, .. } if side == "left" || side == "right");
            if horiz(&places[i].1) {
                let start = i;
                while i < places.len() && horiz(&places[i].1) {
                    i += 1;
                }
                ui.horizontal(|ui| {
                    for (k, _) in &places[start..i] {
                        self.draw(ui, *k, acts);
                    }
                });
            } else {
                let (k, p) = &places[i];
                let (padx, pady) = match p {
                    Place::Pack { padx, pady, .. } => (*padx as f32, *pady as f32),
                    _ => (0.0, 0.0),
                };
                ui.add_space(pady);
                ui.horizontal(|ui| {
                    ui.add_space(padx);
                    ui.vertical(|ui| self.draw(ui, *k, acts));
                });
                ui.add_space(pady);
                i += 1;
            }
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui, w: W, opts: &std::collections::BTreeMap<String, Val>, acts: &mut Vec<Act>) {
        let size = egui::vec2(opts.get("width").map(|v| v.float()).unwrap_or(200.0) as f32, opts.get("height").map(|v| v.float()).unwrap_or(150.0) as f32);
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
        let p = ui.painter_at(rect);
        let bg = opts.get("bg").or(opts.get("background")).and_then(|v| color(&v.text())).unwrap_or(egui::Color32::WHITE);
        p.rect_filled(rect, 0.0, bg);
        let at = |x: f64, y: f64| rect.min + egui::vec2(x as f32, y as f32);
        let items = snapshot(|ws| ws[w].items.clone());
        for it in items {
            let fill = it.opts.get("fill").and_then(|v| color(&v.text()));
            let outline = it.opts.get("outline").map(|v| color(&v.text())).unwrap_or(Some(egui::Color32::BLACK));
            let width = it.opts.get("width").map(|v| v.float() as f32).unwrap_or(1.0);
            let stroke = egui::Stroke::new(width, outline.unwrap_or(egui::Color32::TRANSPARENT));
            let c = &it.coords;
            match it.kind.as_str() {
                "rectangle" if c.len() >= 4 => {
                    let r = egui::Rect::from_two_pos(at(c[0], c[1]), at(c[2], c[3]));
                    if let Some(f) = fill {
                        p.rect_filled(r, 0.0, f);
                    }
                    p.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
                }
                "oval" if c.len() >= 4 => {
                    let r = egui::Rect::from_two_pos(at(c[0], c[1]), at(c[2], c[3]));
                    let pts: Vec<egui::Pos2> = (0..48).map(|k| {
                        let t = k as f32 / 48.0 * std::f32::consts::TAU;
                        r.center() + egui::vec2(r.width() / 2.0 * t.cos(), r.height() / 2.0 * t.sin())
                    }).collect();
                    p.add(egui::Shape::convex_polygon(pts, fill.unwrap_or(egui::Color32::TRANSPARENT), stroke));
                }
                "line" if c.len() >= 4 => {
                    let pts: Vec<egui::Pos2> = c.chunks(2).filter(|q| q.len() == 2).map(|q| at(q[0], q[1])).collect();
                    p.add(egui::Shape::line(pts, egui::Stroke::new(width, fill.unwrap_or(egui::Color32::BLACK))));
                }
                "polygon" if c.len() >= 6 => {
                    let pts: Vec<egui::Pos2> = c.chunks(2).filter(|q| q.len() == 2).map(|q| at(q[0], q[1])).collect();
                    p.add(egui::Shape::convex_polygon(pts, fill.unwrap_or(egui::Color32::TRANSPARENT), stroke));
                }
                "text" if c.len() >= 2 => {
                    let t = it.opts.get("text").map(|v| v.text()).unwrap_or_default();
                    p.text(at(c[0], c[1]), egui::Align2::CENTER_CENTER, t, egui::FontId::proportional(14.0), fill.unwrap_or(egui::Color32::BLACK));
                }
                _ => {}
            }
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let d = pos - rect.min;
                acts.push(Act::Tap(w, d.x as i64, d.y as i64));
            }
        }
    }
}

struct Front(Ui);

impl eframe::App for Front {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let _ = frame;
        run_timers(self.0.started.elapsed().as_millis() as u64);
        if quitting() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let mut acts = vec![];
        let title = snapshot(|ws| ws[0].title.clone());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| self.0.draw(ui, 0, &mut acts));
        });
        // Toplevel windows as egui windows inside the main one.
        let tops: Vec<(W, String)> = snapshot(|ws| ws.iter().enumerate().filter(|(_, w)| w.kind == Kind::Toplevel && w.alive).map(|(i, w)| (i, w.title.clone())).collect());
        for (t, name) in tops {
            egui::Window::new(name).id(egui::Id::new(("top", t))).show(ctx, |ui| self.0.draw(ui, t, &mut acts));
        }
        self.0.dialogs.extend(DIALOGS.with(|d| std::mem::take(&mut *d.borrow_mut())));
        if let Some((t, m)) = self.0.dialogs.first().cloned() {
            let mut close = false;
            egui::Window::new(t).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(m);
                close = ui.button("OK").clicked();
            });
            if close {
                self.0.dialogs.remove(0);
            }
        }
        // Keys for `root.bind(...)`.
        let keys: Vec<String> = ctx.input(|i| i.events.iter().filter_map(|e| match e {
            egui::Event::Key { key, pressed: true, .. } => Some(match key {
                egui::Key::Enter => "Return".to_string(),
                egui::Key::Escape => "Escape".to_string(),
                egui::Key::Space => "space".to_string(),
                egui::Key::ArrowUp => "Up".to_string(),
                egui::Key::ArrowDown => "Down".to_string(),
                egui::Key::ArrowLeft => "Left".to_string(),
                egui::Key::ArrowRight => "Right".to_string(),
                k => k.name().to_lowercase(),
            }),
            _ => None,
        }).collect());
        for k in keys {
            if !acts.iter().any(|a| matches!(a, Act::Key(_, kk) if *kk == k)) {
                acts.push(Act::Key(0, k));
            }
        }
        for a in acts {
            match a {
                Act::Click(w) => invoke(w),
                Act::SetText(w, s) => set_text(w, &s),
                Act::Select(w, i) => {
                    selection_clear(w);
                    selection_set(w, i as i64);
                    fire(w, "<<ListboxSelect>>", &Event { widget: w, ..Default::default() });
                }
                Act::Slide(w, x) => {
                    scale_set(w, x);
                    if let Val::Cb(f) = cget(w, "command") {
                        f();
                    }
                }
                Act::Choose(w, s) => {
                    set_text(w, &s);
                    fire(w, "<<ComboboxSelected>>", &Event { widget: w, ..Default::default() });
                }
                Act::Key(w, k) => {
                    let e = Event { widget: w, keysym: k.clone(), char: if k.chars().count() == 1 { k.clone() } else { String::new() }, x: 0, y: 0 };
                    for n in [format!("<{k}>"), format!("<KeyPress-{k}>"), format!("<Key-{k}>")] {
                        fire(w, &n, &e);
                        if w != 0 {
                            fire(0, &n, &e);
                        }
                    }
                    fire(w, "<Key>", &e);
                }
                Act::Tap(w, x, y) => {
                    let e = Event { widget: w, x, y, ..Default::default() };
                    for n in ["<Button-1>", "<Button>", "<1>", "<ButtonPress-1>"] {
                        fire(w, n, &e);
                    }
                }
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

pub(crate) fn run() {
    let (title, geo) = snapshot(|ws| (ws[0].title.clone(), ws[0].opts.get("geometry").map(|v| v.text())));
    let mut vp = egui::ViewportBuilder::default().with_title(title.clone());
    if let Some(g) = geo {
        let wh: Vec<f32> = g.split(['x', '+']).take(2).filter_map(|s| s.parse().ok()).collect();
        if wh.len() == 2 {
            vp = vp.with_inner_size([wh[0], wh[1]]);
        }
    }
    let opts = eframe::NativeOptions { viewport: vp, ..Default::default() };
    let ui = Ui { started: Instant::now(), edits: Default::default(), dialogs: vec![] };
    let _ = eframe::run_native(&title, opts, Box::new(|_cc| Ok(Box::new(Front(ui)))));
}
