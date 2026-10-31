"""Prints the prompt given to a test-subject model. usage: subject_prompt.py RUN LANG small|big|new"""
import sys
run, lang, size = sys.argv[1:4]
SMALL = "bigadd calc date_diff dijkstra intervals inventory json_sum lru topo wrap anagrams brackets knight ledger log_stats roman_calc schedule spiral".split()
BIG = "access_report bank_api invoices kv_store markdown spreadsheet template timetable".split()
# Held-out big tasks written after v0.15 (ref/gen_big2.py), never used to tune Brevi.
NEW = "sales_report route_planner ini_config stack_vm line_diff room_booking".split()
# Held-out XL tasks written after v0.16 (ref/gen_big3.py); library and project_plan are multi-file.
XL = "order_book mini_sql calc_lang fifo_stock library project_plan".split()
tasks = {"small": SMALL, "big": BIG, "new": NEW, "xl": XL}[size]
B = "/home/user/brevi/bench"; ext = {"brevi": "bv", "python": "py"}[lang]
name = {"brevi": "Brevi", "python": "Python 3 (standard library only)"}[lang]
guide = f"First read the language guide /home/user/brevi/BREVI.md once. When you need details it points to, run {B}/guide.sh {run} WORD to print the guide lines about WORD; this is the only other command you may run. " if lang == "brevi" else ""
print(f"""You are writing small command-line programs in {name}. {guide}Solve these tasks in order: {", ".join(tasks)}.

For each task TASK:
1. Read {B}/tasks/TASK/prompt.md. Programs read stdin and write stdout.
2. Write your whole solution to {B}/work/{run}/TASK.{ext}. If the prompt asks for several files, write them as {B}/work/{run}/TASK/NAME.{ext} instead, with main.{ext} as the entry point.
3. Check it by running exactly: {B}/run.sh {lang} TASK {B}/work/{run}/TASK.{ext} (for a several-file task: {B}/run.sh {lang} TASK {B}/work/{run}/TASK/main.{ext})
   It prints PASS, or the compiler error, or the first failing test.
4. If it did not pass, edit only the lines that need to change (do not rewrite the whole file) and run the check again, up to 6 checks per task, then move on.

Rules: run.sh{" and guide.sh are the only commands" if lang == "brevi" else " is the only command"} you may run. Do not run or test code any other way, and do not read anything else under {B} or the compiler source (no other solutions, tests, logs or reference code). Write code directly, without long planning notes.

When done, reply with one line per task: TASK, PASS or FAIL, number of checks.""")
