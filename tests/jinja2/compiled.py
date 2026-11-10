# Templates compiled to closures: the node kinds and paths the compiler handles itself, each against jinja2 3.1.6.
import jinja2
import markupsafe
from jinja2 import DictLoader, Environment, StrictUndefined

env = Environment(extensions=["jinja2.ext.loopcontrols"])


def show(src, e=None, **ctx):
    try:
        print(repr((e or env).from_string(src).render(**ctx)))
    except Exception as e:
        print(type(e).__name__, e)


# for loops with and without a loop object, else, break and continue, tuple targets
show("{% for x in xs %}{{ x }}{% else %}none{% endfor %}", xs=[1, 2, 3])
show("{% for x in xs %}{{ x }}{% else %}none{% endfor %}", xs=[])
show("{% for x in xs %}{{ loop.index }}/{{ loop.length }}{{ '' if loop.last else ',' }}{% endfor %}", xs="abc")
show("{% for x in xs %}{% if x == 2 %}{% continue %}{% endif %}{{ x }}{% endfor %}", xs=[1, 2, 3])
show("{% for x in xs %}{% if x == 2 %}{% break %}{% endif %}{{ x }}{% endfor %}", xs=[1, 2, 3])
show("{% for x in xs %}{% continue %}{% else %}else ran{% endfor %}", xs=[1, 2])
show("{% for k, v in d.items() %}{{ k }}={{ v }};{% endfor %}", d={"a": 1, "b": 2})
show("{% for a, b in xs %}{{ a }}{{ b }}{% endfor %}", xs=[(1, 2), (3,)])
show("{% for x in xs %}{% for y in ys %}{{ loop.index }}{{ x }}{{ y }} {% endfor %}{% endfor %}", xs=[1, 2], ys="ab")
show("{% for x in xs if x > 1 %}{{ x }}{{ loop.index }}{% endfor %}", xs=[1, 2, 3])
show("{% for x in xs recursive %}{{ x.n }}{% if x.c %}[{{ loop(x.c) }}]{% endif %}{% endfor %}",
     xs=[{"n": 1, "c": [{"n": 2, "c": []}]}, {"n": 3, "c": []}])
show("{% for x in xs %}{{ x }}{% endfor %}{{ x }}", xs=[1, 2])
show("{% for x in undefined_thing %}{{ x }}{% else %}empty{% endfor %}")
show("{% for x in 5 %}{{ x }}{% endfor %}")
show("{% for x in xs %}{% set y = x * 2 %}{{ y }}{% endfor %}[{{ y }}]", xs=[1, 2])
show("{% set ns = namespace(t=0) %}{% for x in xs %}{% set ns.t = ns.t + x %}{% endfor %}{{ ns.t }}", xs=[1, 2, 3])

# include and macros inside a loop see the loop
loader_env = Environment(loader=DictLoader({"item": "<{{ loop is defined }}:{{ x }}>", "loopy": "{{ loop.index }}", "m": "{% macro f(v) %}({{ v }}){% endmacro %}"}))
show("{% for x in xs %}{% include 'item' %}{% endfor %}", loader_env, xs="ab")
show("{% for x in xs %}{% include 'loopy' %}{% endfor %}", loader_env, xs="ab")
show("{% for x in xs %}{% for y in xs %}{% include 'item' %}{% endfor %}{% endfor %}", loader_env, xs="ab")
show("{% set loop = 5 %}{% include 'loopy' %}{% for x in xs %}{% include 'item' %}{% endfor %}", loader_env, xs="a")
show("{% from 'm' import f %}{% for x in xs %}{{ f(x) }}{% endfor %}", loader_env, xs="ab")
show("{% macro g() %}{{ loop.index }}{% endmacro %}{% for x in xs %}{{ g() }}{% endfor %}", loader_env, xs="ab")
show("{% import 'm' as mm %}{% for x in xs %}{{ mm.f(x) }}{% endfor %}", loader_env, xs="ab")

# names: frames, context, globals, undefined
show("{{ a }}{{ b }}{{ c }}", a=1, b=None)
show("{{ range(3) | list }}{{ dict(a=1) }}")
show("{% set a = 5 %}{% with a = 6 %}{{ a }}{% endwith %}{{ a }}", a=1)
strict = Environment(undefined=StrictUndefined)
for src in ["{{ nope }}", "{{ d.nope }}", "{{ d.a }}", "{{ x.y.z }}"]:
    show(src, strict, d={"a": 1})

# attribute lookups on dicts: keys, method names, missing keys, subclasses, non-dicts
class D(dict):
    def __getattr__(self, name):
        return "attr:" + name


class O:
    a = 1
    items = "obj items"


show("{{ d.a }}|{{ d.items is callable }}|{{ d.keys() | list }}|{{ d.missing }}|{{ d['items'] }}|{{ d._x }}",
     d={"a": 1, "items": 2, "_x": 3, "keys": 4})
show("{{ d.a }}|{{ d.zz }}", d=D(a=1))
show("{{ o.a }}|{{ o.items }}|{{ o.b }}", o=O())
show("{{ d.a.b }}|{{ d[0] }}|{{ s[1:3] }}|{{ s[::-1] }}", d={"a": {"b": "deep"}, 0: "zero"}, s="hello")

# filters and tests: no arguments, positional, keyword, *args, **kwargs, context and environment ones
show("{{ x | upper }}{{ x | replace('a', 'b') }}{{ x | truncate(4, killwords=True, end='~') }}", x="banana")
show("{{ xs | join(', ') }}|{{ xs | join(*sep) }}|{{ xs | map('string') | join(**kw) }}", xs=[1, 2], sep=["-"], kw={"d": "+"})
show("{{ x | nofilter }}", x=1)
show("{% if x is divisibleby 3 %}yes{% endif %}{% if x is odd %}odd{% endif %}{{ x is defined }}{{ y is undefined }}", x=9)
show("{{ x is nosuchtest }}", x=1)
env.filters["twice"] = lambda v, n=2: v * n
show("{{ 'ab' | twice }}{{ 'ab' | twice(3) }}")
env.filters["twice"] = lambda v, n=2: "replaced"
show("{{ 'ab' | twice }}")


@jinja2.pass_context
def ctxname(ctx, v):
    return str(v) + ":" + str(ctx.get("who"))


@jinja2.pass_environment
def envname(e, v):
    return str(v) + ":" + type(e).__name__


@jinja2.pass_eval_context
def evname(ec, v):
    return str(v) + ":" + str(ec.autoescape)


env.filters["ctxname"] = ctxname
env.filters["envname"] = envname
env.filters["evname"] = evname
show("{{ 1 | ctxname }}{{ 2 | envname }}{{ 3 | evname }}", who="me")
env.tests["big"] = lambda v, n=10: v > n
show("{{ 11 is big }}{{ 11 is big(20) }}{{ 11 is big(n=5) }}{{ 11 is big(*xs) }}", xs=[30])

# expressions
show("{{ 1 < x < 3 }}{{ 1 < x > 5 }}{{ x in [1, 2] }}{{ x not in [1] }}{{ x == 2 }}{{ x != 2 }}{{ x >= 2 }}{{ x <= 1 }}", x=2)
show("{{ a and b }}|{{ a or b }}|{{ not a }}|{{ (a, b) }}|{{ [a, b] }}|{{ {'k': a} }}", a=0, b="b")
show("{{ 'x' if a else 'y' }}|{{ 'x' if a }}|{{ a ~ b ~ 3 }}", a=0, b="b")
show("{{ 7 + 2 }}{{ 7 - 2 }}{{ 7 * 2 }}{{ 7 / 2 }}{{ 7 // 2 }}{{ 7 % 2 }}{{ 7 ** 2 }}{{ -x }}{{ +x }}", x=4)
show("{{ 'a' * 3 }}{{ [1] + [2] }}{{ 'a' + 1 }}")
show("{{ f(1, b=2) }}{{ f(*xs) }}{{ f(**kw) }}", f=lambda a=0, b=0: a * 10 + b, xs=[3, 4], kw={"b": 5})
show("{{ f(1, **kw) }}", f=lambda a=0, b=0: a, kw={"a": 2})
show("{{ none }}{{ true }}{{ False }}{{ 1.5 }}{{ 'q' }}{{ x }}", x=[1, "a"])

# output: autoescape, finalize, Markup values, non-str values
ae = Environment(autoescape=True)
print(repr(ae.from_string("{{ x }}{{ y }}{{ x ~ y }}{{ '<b>' }}{% autoescape false %}{{ x }}{% endautoescape %}").render(x="<i>", y=markupsafe.Markup("<u>"))))
fin = Environment(finalize=lambda v: "" if v is None else v)
print(repr(fin.from_string("[{{ x }}][{{ y }}][{{ 0 }}]").render(x=None, y="v")))


@jinja2.pass_context
def cfin(ctx, v):
    return "<" + str(v) + ">"


cf = Environment(finalize=cfin)
print(repr(cf.from_string("{{ 1 }}{% for x in xs %}{{ x }}{% endfor %}").render(xs=[2])))

# blocks, extends and super with compiled bodies
ext = Environment(loader=DictLoader({
    "base": "A{% block b %}base{{ x }}{% endblock %}Z{{ self.b() }}{% for i in xs %}{{ i }}{% endfor %}",
    "child": "{% extends 'base' %}ignored{{ x }}{% block b %}[{{ super() }}]{% endblock %}",
    "scoped": "{% for i in xs %}{% block s scoped %}<{{ i }}>{% endblock %}{% endfor %}",
}))
print(repr(ext.get_template("child").render(x=1, xs=[7, 8])))
print(repr(ext.get_template("scoped").render(xs=[1, 2])))

# the same template rendered again, and with other variables
t = env.from_string("{% for m in ms %}{{ m.role }}:{{ m.content | trim }}{% if not loop.last %}\n{% endif %}{% endfor %}")
print(repr(t.render(ms=[{"role": "u", "content": " hi "}, {"role": "a", "content": "yo"}])))
print(repr(t.render(ms=[])))
print(repr(t.render(ms=[{"role": "x"}])))
print(t.module is not None)
