import tkinter as tk
from tkinter import messagebox

root = tk.Tk()
root.title("Counter")
count = tk.IntVar(value=0)
tk.Label(root, textvariable=count).pack()
name = tk.Entry(root)
name.pack()
greet = tk.Label(root, text="")
greet.pack()

def inc():
    count.set(count.get() + 1)

def hello():
    greet.config(text=f"Hello, {name.get()}!")

tk.Button(root, text="+1", command=inc).pack(side="left")
b2 = tk.Button(root, text="Greet", command=hello)
b2.pack(side="left")
name.bind("<Return>", lambda e: b2.invoke())
root.after(1000, lambda: count.set(100))
lb = tk.Listbox(root)
lb.pack()
for s in ["red", "green", "blue"]:
    lb.insert(tk.END, s)
cv = tk.Canvas(root, width=100, height=50)
cv.pack()
cv.create_rectangle(10, 10, 40, 30, fill="red")
if messagebox.askyesno("Start", "Ready?"):
    print("ready")
root.mainloop()
print("bye, count", count.get())
