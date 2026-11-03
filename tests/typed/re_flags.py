import re
TOK = re.compile(r'\{\{(.*?)\}\}', re.S)
LINE = re.compile(r'^(\w+):\s*(.*)$', re.M)
def run(text):
    print(TOK.findall(text))
    print([m.group(1) + "=" + m.group(2) for m in LINE.finditer(text)])
    print(re.split(r'^-+$', text, flags=re.M) if False else re.split(r'\n-+\n', text))
    print(re.findall(r'a.b', 'a\nb axb', re.S), re.findall(r'a.b', 'a\nb axb'))
    print(re.findall(r'^x', 'x\nx', re.M | re.I), re.findall(r'X$', 'x\nX\n', re.I | re.M), re.findall(r'end$', 'the end\n'))
    print(bool(re.search(r'z$', 'z\n')), bool(re.search(r'z\Z', 'z\n')), bool(re.fullmatch(r'z', 'z\n')))
    m = re.match(r'(?s)(.*)!', 'hi\nthere!')
    print(m.group(1) if m else None)
run("name: {{ a\nb }} x\n---\nkey: v {{c}}\nother: 2")
