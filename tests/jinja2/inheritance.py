# Template inheritance, includes, imports and macros across a DictLoader / ChoiceLoader / PrefixLoader.
from jinja2 import Environment, DictLoader, ChoiceLoader, PrefixLoader, FunctionLoader, TemplateNotFound

base = {
    "base.txt": "{% block head %}HEAD{% endblock %}\n{% block body %}{% endblock %}\n{% block foot scoped %}{% for i in range(2) %}[{% block item scoped %}{{ i }}{% endblock %}]{% endfor %}{% endblock %}",
    "child.txt": "{% extends 'base.txt' %}{% block head %}{{ super() }}+child{% endblock %}{% block body %}body of {{ name }}{% endblock %}{% block item %}<{{ super() }}>{% endblock %}",
    "grand.txt": "{% extends 'child.txt' %}{% block head %}{{ super() }}+grand{% endblock %}{% block body %}{{ self.head() }} / {{ super() }}{% endblock %}",
    "dyn.txt": "{% extends layout %}{% block body %}dynamic {{ layout }}{% endblock %}",
    "macros.txt": "{% macro field(name, value='', type='text') -%}<input type=\"{{ type }}\" name=\"{{ name }}\" value=\"{{ value }}\">{%- endmacro %}{% macro box(title) -%}[{{ title }}|{{ caller() }}|{{ varargs }}|{{ kwargs }}]{%- endmacro %}{% set version = '1.2' %}",
    "page.txt": "{% import 'macros.txt' as m %}{% from 'macros.txt' import field, version as v %}{{ m.field('user') }} {{ field('pw', type='password') }} v{{ v }} {{ m.version }}\n{% call m.box('T') %}inside {{ name }}{% endcall %}\n{% include 'part.txt' %} {% include ['nope.txt', 'part.txt'] %} {% include 'missing.txt' ignore missing %}|",
    "part.txt": "part({{ name }})",
    "loopy.txt": "{% for x in items %}{% include 'item.txt' %}{% endfor %}",
    "item.txt": "<{{ x }}>",
    "ctx.txt": "{% from 'shows.txt' import show with context %}{% import 'shows.txt' as s %}{{ show() }} {{ s.show() }}",
    "shows.txt": "{% macro show() %}name={{ name|default('none') }}{% endmacro %}",
}
env = Environment(loader=ChoiceLoader([DictLoader(base), PrefixLoader({"lib": DictLoader({"x.txt": "lib x {{ name }}"})}),
                                       FunctionLoader(lambda n: "fn:" + n if n.startswith("fn") else None)]))
for name in ["child.txt", "grand.txt", "page.txt", "loopy.txt", "ctx.txt", "lib/x.txt", "fn_one"]:
    print(name, "=>", repr(env.get_template(name).render(name="ada", items=["a", "b"])))
print(repr(env.get_template("dyn.txt").render(layout="base.txt")))
print(repr(env.get_template("dyn.txt").render(layout=env.get_template("child.txt"), name="n")))
print(env.select_template(["no.txt", "part.txt"]).render(name="sel"), env.get_or_select_template("part.txt").name)
print(sorted(Environment(loader=DictLoader(base)).list_templates())[:5], env.get_template("page.txt") is env.get_template("page.txt"))
for bad in ["nothere.txt", "lib/none.txt"]:
    try:
        env.get_template(bad)
    except TemplateNotFound as e:
        print("TemplateNotFound", e, e.name, e.templates)
try:
    env.select_template(["a.txt", "b.txt"])
except Exception as e:
    print(type(e).__name__, e)
mod = env.get_template("macros.txt").module
print(mod.field("q", 3), mod.version, str(mod)[:40])
print(env.get_template("macros.txt").make_module({"x": 1}).version)
m = env.get_template("macros.txt").module.field
print(m.name, m.arguments, m.catch_kwargs, m.catch_varargs, m.caller)
t = env.from_string("{% macro rec(n) %}{% if n %}{{ n }}{{ rec(n - 1) }}{% endif %}{% endmacro %}{{ rec(5) }}|{% set x = 1 %}{% macro sx() %}{{ x }}{% endmacro %}{% set x = 2 %}{{ sx() }}")
print(t.render(), t.module.rec(3))
print(env.from_string("{% set a, b = 1, 2 %}{% set c %}cap{{ a }}{% endset %}{{ a }}{{ b }}{{ c }}|{% filter upper %}up {{ c }}{% endfilter %}|{% with q = 5, r = 6 %}{{ q + r }}{% endwith %}{{ q }}").render())
print(env.from_string("{% for a, b in [(1, 2), (3, 4)] if a > 1 %}{{ a }}{{ b }}{{ loop.length }}{% else %}none{% endfor %}{% for x in [] %}{% else %}empty{% endfor %}").render())
print(env.from_string("{% for row in rows recursive %}{{ row.n }}{% if row.kids %}({{ loop(row.kids) }}){% endif %}{{ loop.depth }}{% endfor %}").render(rows=[{"n": 1, "kids": [{"n": 2, "kids": [{"n": 3}]}]}, {"n": 4}]))
print(env.from_string("{% for x in 'abcd' %}{{ loop.cycle('o', 'e') }}{{ loop.previtem|default('-') }}{{ loop.nextitem|default('-') }}{{ loop.revindex }}{{ loop.changed(x in 'ab') }} {% endfor %}").render())
