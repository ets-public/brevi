# A small MCP server over SQLite: notes an agent can add, search and query.
# Plain Python on the official `mcp` SDK; `brevi build sqlite_mcp.py --release` makes it one binary.
import sqlite3
import sys

from mcp.server.mcpserver import MCPServer

DB = sys.argv[1] if len(sys.argv) > 1 else "notes.db"
server = MCPServer("sqlite-notes")


def connect():
    # one connection per call: the SDK may run a sync tool on a worker thread
    db = sqlite3.connect(DB)
    db.execute("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, title TEXT NOT NULL, body TEXT NOT NULL)")
    return db


@server.tool()
def add_note(title: str, body: str) -> int:
    """Save a note and return its id."""
    with connect() as db:
        return db.execute("INSERT INTO notes (title, body) VALUES (?, ?)", (title, body)).lastrowid


@server.tool()
def search_notes(text: str, limit: int = 10) -> list[dict]:
    """Notes whose title or body contains the text, newest first."""
    with connect() as db:
        rows = db.execute(
            "SELECT id, title, body FROM notes WHERE title LIKE ?1 OR body LIKE ?1 ORDER BY id DESC LIMIT ?2",
            (f"%{text}%", limit),
        ).fetchall()
    return [{"id": i, "title": t, "body": b} for i, t, b in rows]


@server.tool()
def query(sql: str) -> list[list]:
    """Run one read-only SQL statement and return its rows."""
    if not sql.lstrip().lower().startswith(("select", "with")):
        raise ValueError("only SELECT or WITH queries are allowed")
    with connect() as db:
        return [list(r) for r in db.execute(sql).fetchall()]


if __name__ == "__main__":
    server.run()
