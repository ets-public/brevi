import re
print(bool(re.match(r'\N{LESS-THAN SIGN}', '<')), bool(re.match(r'\N{less-than sign}', '<')), re.match(r'\N{LESS-THAN SIGN}', '>'))
print(bool(re.match(r'\N{SNAKE}', '\U0001f40d')), bool(re.match(r'[\N{LESS-THAN SIGN}-\N{GREATER-THAN SIGN}]', '=')))
for p in [r'\N', r'[\N]', r'\N{', r'[\N{', r'\N{}', r'[\N{}]', r'\NSNAKE}', r'[\NSNAKE}]', r'\N{SNAKE', r'[\N{SNAKE]', r'[\N{SNAKE]}', r'\N{SPAM}', r'[\N{SPAM}]', r'\N{KEYCAP NUMBER SIGN}', r'(?x)\N{ SNAKE}', r'a\N{NBSP}b']:
    try: print(p, re.compile(p).pattern)
    except re.error as e: print(p, e.msg, e.pos)
for p in [br'\N{LESS-THAN SIGN}', br'[\N{LESS-THAN SIGN}]']:
    try: re.compile(p)
    except re.error as e: print(p, e.msg, e.pos)
print(re.findall(r'\N{CJK UNIFIED IDEOGRAPH-4E00}|\N{hangul syllable gag}', 'x一각y'))
