"""Prints the prompt given to a test-subject model. usage: subject_prompt.py RUN LANG small|big"""
import sys
run, lang, size = sys.argv[1:4]
SMALL = "bigadd calc date_diff dijkstra intervals inventory json_sum lru topo wrap anagrams brackets knight ledger log_stats roman_calc schedule spiral".split()
BIG = "access_report bank_api invoices kv_store markdown spreadsheet template timetable".split()
tasks = SMALL if size == "small" else BIG
B = "/home/user/brevi/bench"; ext = {"brevi": "bv", "python": "py"}[lang]
name = {"brevi": "Brevi", "python": "Python 3 (standard library only)"}[lang]
guide = f"First read the language guide /home/user/brevi/BREVI.md once. " if lang == "brevi" else ""
print(f"""You are writing small command-line programs in {name}. {guide}Solve these tasks in order: {", ".join(tasks)}.

For each task TASK:
1. Read {B}/tasks/TASK/prompt.md. Programs read stdin and write stdout.
2. Write your whole solution to {B}/work/{run}/TASK.{ext}.
3. Check it by running exactly: {B}/run.sh {lang} TASK {B}/work/{run}/TASK.{ext}
   It prints PASS, or the compiler error, or the first failing test.
4. If it did not pass, fix the file and run the check again, up to 6 checks per task, then move on.

Rules: run.sh is the only command you may run. Do not run or test code any other way, and do not read anything else under {B} or the compiler source (no other solutions, tests, logs or reference code). Write code directly, without long planning notes.

When done, reply with one line per task: TASK, PASS or FAIL, number of checks.""")
