# Writes tasks/<name>/{prompt.md,tests/N.in,N.out} for the edit benchmark, using ref/<name>.py.
import os, subprocess
SETUP = "add A1 apple 0.50 100\nadd B2 bread 2.5 20\nadd C3 cheese 12.99 10\nadd D4 dates 7 30\n"
T = {
 "discount": ("Orders whose total is 100.00 or more get 10% off. The discount is rounded down to the cent. The discounted total is what `order` prints and what `revenue` counts.",
   [SETUP + "order o1 C3 8\norder o2 A1 2\nrevenue\n",
    SETUP + "order o1 D4 10 C3 3 A1 1\ncancel o1\norder o2 C3 7 A1 1\nrevenue\nreport\n",
    SETUP + "order o1 A1 100 B2 20\norder o2 C3 1\nrevenue\n"]),
 "top": ("Add a command `top N`. It prints the N products with the most units sold in orders that are not cancelled, one per line as `SKU UNITS`, most units first, ties broken by SKU in alphabetical order. Products with no units sold are not printed.",
   [SETUP + "order o1 A1 5 B2 2\norder o2 B2 3 C3 1\ntop 2\n",
    SETUP + "order o1 A1 5\norder o2 D4 5\norder o3 C3 9\ncancel o3\ntop 5\n",
    SETUP + "top 3\norder o1 B2 1\ntop 1\nreport\n"]),
 "cancel_twice": ("Bug report: cancelling an order twice puts its stock back twice. A second `cancel` of the same order must print `error: order ID already cancelled` (with the order's id) and change nothing.",
   [SETUP + "order o1 A1 10\ncancel o1\ncancel o1\nreport\n",
    SETUP + "order o1 C3 5\norder o2 C3 5\ncancel o2\ncancel o2\nrevenue\nreport\n",
    SETUP + "cancel o9\norder o1 B2 1\ncancel o1\nrevenue\n"]),
 "customer": ("Orders now name a customer: the command becomes `order ID CUSTOMER SKU QTY [SKU QTY ...]`. Add a command `customer NAME` that prints `NAME COUNT TOTAL`: how many orders that customer has that are not cancelled, and their total in money format (`0.00` if none). Everything else works as before.",
   [SETUP + "order o1 ann A1 2\norder o2 bob B2 1\norder o3 ann C3 1\ncustomer ann\ncustomer bob\n",
    SETUP + "order o1 ann A1 2\ncancel o1\ncustomer ann\ncustomer zed\nrevenue\n",
    SETUP + "order o1 cy D4 2 A1 4\norder o2 cy C3 11\ncustomer cy\nreport\n"]),
}
for name, (prompt, ins) in T.items():
    d = f"tasks/{name}/tests"; os.makedirs(d, exist_ok=True)
    open(f"tasks/{name}/prompt.md", "w").write(prompt + "\n")
    for i, s in enumerate(ins, 1):
        out = subprocess.run(["python3", f"ref/{name}.py"], input=s, capture_output=True, text=True).stdout
        open(f"{d}/{i}.in", "w").write(s); open(f"{d}/{i}.out", "w").write(out.rstrip("\n"))
        print(name, i, out.replace("\n", " | ")[:150])
