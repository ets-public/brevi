// What Brevi emits for a small tkinter program (by hand, until the compiler does it):
//   root = tk.Tk(); root.title("Counter"); count = tk.IntVar(value=0)
//   tk.Label(root, textvariable=count).pack()
//   name = tk.Entry(root); name.pack()
//   greet = tk.Label(root, text=""); greet.pack()
//   def inc(): count.set(count.get() + 1)
//   def hello(): greet.config(text=f"Hello, {name.get()}!")
//   tk.Button(root, text="+1", command=inc).pack(side="left")
//   tk.Button(root, text="Greet", command=hello).pack(side="left")
//   root.after(1000, lambda: count.set(100)); root.mainloop()
use brevi_tk::*;
use std::rc::Rc;

fn main() {
    let root = tk();
    title(root, "Counter");
    let count = var("IntVar", Some(0.into()));
    let l = widget("Label", root, vec![("textvariable", Val::Var(count))]);
    pack(l, vec![]);
    let name = widget("Entry", root, vec![]);
    pack(name, vec![]);
    let greet = widget("Label", root, vec![("text", "".into())]);
    pack(greet, vec![]);
    let inc: Cb = Rc::new(move || var_set(count, var_get_int(count) + 1));
    let hello: Cb = Rc::new(move || config(greet, vec![("text", format!("Hello, {}!", get(name)).into())]));
    let b1 = widget("Button", root, vec![("text", "+1".into()), ("command", Val::Cb(inc))]);
    pack(b1, vec![("side", "left".into())]);
    let b2 = widget("Button", root, vec![("text", "Greet".into()), ("command", Val::Cb(hello))]);
    pack(b2, vec![("side", "left".into())]);
    bind(name, "<Return>", Rc::new(move |_e: &Event| invoke(b2)));
    after(1000, Rc::new(move || var_set(count, 100)));
    let lb = widget("Listbox", root, vec![]);
    pack(lb, vec![]);
    for s in ["red", "green", "blue"] {
        insert(lb, "end".into(), s);
    }
    let cv = widget("Canvas", root, vec![("width", 100.into()), ("height", 50.into())]);
    pack(cv, vec![]);
    create(cv, "rectangle", vec![10.0, 10.0, 40.0, 30.0], vec![("fill", "red".into())]);
    if messagebox("askyesno", "Start", "Ready?").truthy() {
        println!("ready");
    }
    mainloop();
    println!("bye, count {}", var_get_int(count));
}
