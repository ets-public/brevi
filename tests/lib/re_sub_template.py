# re.sub replacement templates on the typed build: escapes such as \n and \b, \g<1>, octal \101 and \0.
import re
print(repr(re.sub(r'\r\n', r'\n', 'abc\r\ndef\r\n')))
print(repr(re.sub(r'(x)', r'\1\1\b', 'xx')))
print(repr(re.sub(r'(x)', r'[\g<1>]\101\0', 'xx')))
print(re.sub(r'(\w+) (\w+)', r'\2 \1', 'hello world'))
