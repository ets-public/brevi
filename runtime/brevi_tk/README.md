# brevi_tk

The GUI runtime that Brevi compiles Python `tkinter` programs onto. The compiler (`src/gui.rs`, glue in `src/gui_glue.rs`) pastes this crate's source into the program, so a tkinter program needs no download; compiled programs run headless unless built with `brevi build --window`, which also pastes the egui front end and adds `eframe`.

- Widgets are plain handles into one table, so Brevi's value semantics need no shared objects for them: `Tk`, `Toplevel`, `Frame`, `LabelFrame`, `Label`, `Button`, `Entry`, `Text`, `Checkbutton`, `Radiobutton`, `Listbox`, `Scale`, `Spinbox`, `Combobox`, `Canvas` (rectangle, oval, line, polygon, text), `Progressbar`, `Separator`; `pack`, `grid`, `place`; `StringVar`, `IntVar`, `BooleanVar`, `DoubleVar` with traces; `after` timers; `bind`; `messagebox`.
- With feature `window`, a real window through egui (pure Rust; one binary on Windows, macOS and Linux).
- Headless (no feature, `BREVI_GUI=headless`, or no display on Linux): `mainloop` reads actions from stdin and prints the window as text, so a program's GUI can be tested like any other stdin/stdout program, and by AI models that cannot see a screen:

```
$ printf 'click +1\ntype 1 Adam\nkey 1 Return\nshow\n' | cargo run -q --example counter
window "Counter"
  Label "1"
  Entry "Adam"
  Label "Hello, Adam!"
  Button "+1"
  Button "Greet"
  ...
```

Actions: `show`, `click TEXT`, `type N TEXT`, `key N KEY`, `select N I`, `slide N X`, `tap N X Y`, `wait MS`, `answer yes|no|TEXT`, `close` (see `src/headless.rs`).
