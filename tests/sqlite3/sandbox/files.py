import sqlite3

con = sqlite3.connect("notes.db")
con.execute("create table if not exists n(t)")
