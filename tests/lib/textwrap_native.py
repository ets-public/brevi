import textwrap
from textwrap import dedent, fill, wrap

text = ("The quick brown fox jumps over the lazy dog. A well-known, state-of-the-art "
        "algorithm--really--works; supercalifragilisticexpialidocious words break too.\tTabs\nand newlines.")
for w in (10, 15, 20, 33, 70):
    print(w, wrap(text, w))
print(fill(text, 25, initial_indent="* ", subsequent_indent="  "))
print(wrap(text, 12, break_long_words=False))
print(wrap(text, 12, break_on_hyphens=False))
print(wrap("  leading and   inner   spaces  ", 10, drop_whitespace=False))
print(wrap("a-b-c-d-e-f e-mail co-op x--y --z long-hyphenated-word", 6))
print(textwrap.shorten("Hello  world, this is a long sentence to cut", width=20))
print(textwrap.shorten("Hello world", width=11), textwrap.shorten("Supercalifragilistic word", 10, placeholder="..."))
print(wrap(text, 30, max_lines=2), wrap(text, 30, max_lines=3, placeholder=" ~"))
print(repr(dedent("""
    def f():
        return 1
      
    x = 2
""")))
print(repr(dedent("\ta\n\t  b\n\n\tc")), repr(dedent("  a\n b")))
print(repr(textwrap.indent("one\n\ntwo\n", "> ")), repr(textwrap.indent("one\n\ntwo", "+", lambda l: True)))
tw = textwrap.TextWrapper(width=18, subsequent_indent="    ")
print(tw.wrap("Lorem ipsum dolor sit amet, consectetur adipiscing elit."), tw.fill("short"))
print(wrap("", 10), wrap("   ", 10), fill("x" * 25, 10))
try:
    wrap("abc", 0)
except ValueError as e:
    print("ValueError:", e)
