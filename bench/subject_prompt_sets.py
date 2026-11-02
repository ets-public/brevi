"""The benchmark task sets, shared by subject_prompt.py and e2e.py."""
SMALL = "bigadd calc date_diff dijkstra intervals inventory json_sum lru topo wrap anagrams brackets knight ledger log_stats roman_calc schedule spiral".split()
BIG = "access_report bank_api invoices kv_store markdown spreadsheet template timetable".split()
# Held-out big tasks written after v0.15 (ref/gen_big2.py), never used to tune Brevi.
NEW = "sales_report route_planner ini_config stack_vm line_diff room_booking".split()
# Held-out XL tasks written after v0.16 (ref/gen_big3.py); library and project_plan are multi-file.
XL = "order_book mini_sql calc_lang fifo_stock library project_plan".split()
# Second held-out XL set, written after v0.18 (ref/gen_big4.py); tiny_cpu and parking are multi-file.
XL2 = "grep_lite payroll league crafting tiny_cpu parking".split()
# Third held-out XL set (ref/gen_big5.py).
XL3 = "books toc poker trains quiz warehouse".split()
# "Huge" held-out set (ref/gen_huge.py, 1 Nov 2026): 300-800 line programs over 3-4 files, for the big-programs push.
HUGE = "minilang chess adventure".split()
# GUI set (ref/gen_gui.py, 1 Nov 2026): tkinter apps tested headlessly (tkshim/ for CPython, runtime/brevi_tk for Brevi).
GUI = "todo_app calc_app tictactoe_app".split()
SETS = {"gui": GUI, "small": SMALL, "big": BIG, "new": NEW, "xl": XL, "xl2": XL2, "xl3": XL3, "huge": HUGE}
