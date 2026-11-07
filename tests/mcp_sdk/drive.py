# Drives an MCP server over stdio in lockstep, the way a client does: each request is sent once the reply to
# the one before it is in, so the transcript does not depend on timing. Used to compare Brevi's build of a
# server with CPython running it.
#
#   python3 drive.py SCRIPT -- COMMAND...
#
# SCRIPT holds one JSON-RPC message per line, sent as written. A message with an "id" waits for the reply
# with that id; one without (or with a null id) waits for nothing. Other lines:
#   #raw TEXT     send TEXT as a line, as is (bad JSON); takes the one line back, if one comes within 2 s
#   #wait N       read N more lines (server notifications sent after a reply)
#   #nowait MSG   send MSG and go on without waiting for its reply (cancellation, requests in flight)
#   #close        close the server's stdin and wait for it to exit
#   # anything    a comment
# The transcript (what was sent, what came back, the exit code, then stderr) goes to stdout.
import json
import os
import subprocess
import sys
import threading
import queue

TIMEOUT = float(os.environ.get("DRIVE_TIMEOUT", "20"))


def main():
    sep = sys.argv.index("--")
    script = open(sys.argv[1]).read().splitlines()
    cmd = sys.argv[sep + 1:]
    p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    lines = queue.Queue()
    err = []

    def rd():
        for l in p.stdout:
            lines.put(l.decode("utf-8", "replace").rstrip("\n"))
        lines.put(None)

    def rderr():
        for l in p.stderr:
            err.append(l.decode("utf-8", "replace").rstrip("\n"))

    threading.Thread(target=rd, daemon=True).start()
    te = threading.Thread(target=rderr, daemon=True)
    te.start()
    out = []

    def get(t=TIMEOUT, quiet=False):
        try:
            l = lines.get(timeout=t)
        except queue.Empty:
            out.append("! none" if quiet else "! timeout")
            return None if quiet else False
        if l is None:
            out.append("! eof")
            return False
        out.append("< " + l)
        return l

    def send(text):
        out.append("> " + text)
        try:
            p.stdin.write(text.encode() + b"\n")
            p.stdin.flush()
        except BrokenPipeError:
            out.append("! broken pipe")

    alive = True
    for line in script:
        if not line.strip() or (line.startswith("#") and not line.startswith(("#raw ", "#wait ", "#close", "#nowait "))):
            continue
        if not alive:
            break
        if line.startswith("#raw "):
            send(line[5:])
            alive = get(2, True) is not False
            continue
        if line.startswith("#wait "):
            for _ in range(int(line[6:])):
                if get() is False:
                    alive = False
                    break
            continue
        if line.startswith("#close"):
            break
        if line.startswith("#nowait "):
            send(line[8:])
            continue
        msg = json.loads(line)
        send(line)
        if not isinstance(msg, dict) or msg.get("id") is None or "method" not in msg:
            continue
        while True:
            l = get()
            if l is False:
                alive = False
                break
            try:
                r = json.loads(l)
            except ValueError:
                continue
            if isinstance(r, dict) and r.get("id") == msg["id"] and "method" not in r:
                break
    try:
        p.stdin.close()
    except BrokenPipeError:
        pass
    try:
        code = p.wait(timeout=TIMEOUT)
    except subprocess.TimeoutExpired:
        p.kill()
        code = "killed"
    while True:
        try:
            l = lines.get(timeout=1)
        except queue.Empty:
            break
        if l is None:
            break
        out.append("< " + l)
    te.join(timeout=2)
    out.append("exit %s" % code)
    for l in err:
        out.append("stderr: " + l)
    print("\n".join(out))


main()
