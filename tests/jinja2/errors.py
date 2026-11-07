# The errors prompt code catches: syntax errors with line numbers, undefined values, missing templates, bad filters.
from jinja2 import (Environment, DictLoader, StrictUndefined, ChainableUndefined, DebugUndefined, Undefined,
                    TemplateSyntaxError, UndefinedError, TemplateNotFound, TemplateRuntimeError, TemplateAssertionError,
                    TemplateError)

env = Environment(loader=DictLoader({"bad.txt": "line1\n{% if x %}\nno end", "ok.txt": "{{ y.z }}"}))
for src in ["{% for %}", "{{ 1 + }}", "{% if a %}x", "{% endfor %}", "{{ x|nofilter }}", "{% block a %}{% endblock %}{% block a %}{% endblock %}",
            "{{ x is notatest }}", "{% macro m(a=1, b) %}{% endmacro %}", "{% extends 'a' %}{% extends 'b' %}x", "{{ 'unclosed }}",
            "{% set loop = 1 %}", "{% raw %}never closed", "{{ a.0.b }}{{ }}", "{% include %}", "{%- for x in y -%}{{ x }}{%- endfor -%}{% endif %}",
            "{{ (1, 2 }}", "{% foo bar %}", "{{ x[ }}", "line\n\n{{ x | }}", "{% for x in y %}{% break %}{% endfor %}"]:
    try:
        Environment().from_string(src)
        print("ok", repr(src))
    except TemplateSyntaxError as e:
        print(type(e).__name__, "|", e.message, "|", e.lineno, "|", e.name, "|", str(e).splitlines()[0])
    except TemplateAssertionError as e:
        print("assert", e)
try:
    env.get_template("bad.txt")
except TemplateSyntaxError as e:
    print(e.message, e.lineno, e.name, e.filename, "|", str(e))
for U in [Undefined, StrictUndefined, ChainableUndefined, DebugUndefined]:
    e = Environment(undefined=U)
    for src in ["[{{ x }}]", "[{{ x.y }}]", "[{{ d.nokey }}]", "[{{ x|default('dflt') }}]", "[{{ x is defined }}]", "[{% if x %}t{% else %}f{% endif %}]",
                "[{{ x ~ 'a' }}]", "[{{ x|length if false else '-' }}]", "[{% for i in x %}{{ i }}{% endfor %}]", "[{{ x + 1 }}]",
                "[{{ d['nokey'] }}]", "[{{ l[5] }}]", "[{{ x|list }}]", "[{{ x == none }}]", "[{{ x.y.z }}]"]:
        try:
            print(U.__name__, src, "=>", repr(e.from_string(src).render(d={"k": 1}, l=[1])))
        except UndefinedError as err:
            print(U.__name__, src, "=> UndefinedError:", err)
        except TypeError as err:
            print(U.__name__, src, "=> TypeError:", err)
try:
    env.get_template("missing.txt")
except TemplateNotFound as e:
    print("TemplateNotFound", e.message, isinstance(e, IOError), isinstance(e, LookupError), isinstance(e, TemplateError))
try:
    Environment().from_string("{{ x() }}").render(x=lambda: 1 / 0)
except ZeroDivisionError as e:
    print("ZeroDivisionError", e)
try:
    Environment().from_string("{{ 'a' + 1 }}").render()
except TypeError as e:
    print("TypeError", e)
try:
    Environment().from_string("{% extends 'x' %}").render()
except TypeError as e:
    print("TypeError", e)
try:
    Environment().from_string("{{ x.__class__ }}{{ cycler().reset() }}").render(x=1)
    print("ok attr")
except Exception as e:
    print(type(e).__name__, e)
print(Environment(undefined=StrictUndefined).from_string("{{ x|default('safe') }}").render())
u = Undefined(name="nm")
print(repr(str(u)), bool(u), len(u), list(u), u == Undefined(), repr(u), u is not None)
print(repr(StrictUndefined(name="q", hint="custom hint")._undefined_message))
try:
    str(StrictUndefined(name="q"))
except UndefinedError as e:
    print("strict str:", e)
try:
    Environment().from_string("{{ x.y }}").render()
except UndefinedError as e:
    print("default attr of undefined:", e)
try:
    Environment().from_string("{{ d.a.b }}").render(d={})
except UndefinedError as e:
    print("dict:", e)
try:
    Environment().from_string("{{ o.missing.x }}").render(o=type("Obj", (), {})())
except UndefinedError as e:
    print("object:", e)
