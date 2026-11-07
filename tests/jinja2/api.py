# The Environment API agent code leans on: custom filters, tests and globals, pass_context, compile_expression,
# finalize, line statements, custom delimiters, Template() directly, and render with a dict.
from jinja2 import Environment, Template, DictLoader, pass_context, pass_eval_context, pass_environment, StrictUndefined
from jinja2.runtime import Context
from jinja2.utils import Cycler, Joiner, Namespace
import jinja2
import markupsafe


def shout(s, n=1):
    return s.upper() + "!" * n


@pass_context
def who(ctx, value):
    return value + "@" + ctx.get("user", "anon") + "/" + str(ctx.name)


@pass_eval_context
def ae(ectx, value):
    return value + ("[auto]" if ectx.autoescape else "[raw]")


@pass_environment
def delims(env, value):
    return env.variable_start_string + value + env.variable_end_string


def is_prime(n):
    return n > 1 and all(n % d for d in range(2, n))


@pass_context
def lookup(ctx, key):
    return ctx.resolve(key)


env = Environment(loader=DictLoader({"t.txt": "{{ 'x'|who }}"}))
env.filters.update(shout=shout, who=who, ae=ae, delims=delims)
env.tests["prime"] = is_prime
env.globals.update(site="brevi", lookup=lookup, add=lambda a, b: a + b)
print(env.from_string("{{ 'hi'|shout }} {{ 'hi'|shout(3) }} {{ 'v'|who }} {{ 'v'|ae }} {{ 'v'|delims }}").render(user="ada"))
print(env.get_template("t.txt").render(user="bob"), env.from_string("{{ 'x'|who }}").render())
print(env.from_string("{{ range(20)|select('prime')|list }} {{ 7 is prime }} {{ site }} {{ lookup('site') }} {{ add(2, 3) }}").render())
print(env.from_string("{{ g }}", globals={"g": "tmpl-global"}).render(), env.from_string("{{ g }}").render(g="ctx"))
f = env.compile_expression("a + b * 2")
print(f(a=1, b=2), env.compile_expression("items|map('upper')|list")(items=["a", "b"]), env.compile_expression("x.y")(x={}),
      env.compile_expression("x.y", undefined_to_none=False)(x={}) is None)
print(Template("Hello {{ name }}! {{ 1 + 2 }}").render(name="World"), Template("{{ a }}").render({"a": 1}), Template("{{ a }}{{ b }}").render({"a": 1}, b=2))
t = Template("{% set x = 42 %}{% macro hi(n) %}hi {{ n }}{% endmacro %}{{ x }}", trim_blocks=True)
print(t.render(), t.module.x, t.module.hi("m"), t.environment.trim_blocks)
fin = Environment(finalize=lambda v: "" if v is None else v)
print(repr(fin.from_string("[{{ none }}][{{ 0 }}][{{ x }}]").render(x=None)))
ls = Environment(line_statement_prefix="#", line_comment_prefix="##")
print(repr(ls.from_string("# for i in range(3)\n- {{ i }} ## note\n# endfor\ndone").render()))
cd = Environment(block_start_string="<%", block_end_string="%>", variable_start_string="${", variable_end_string="}", comment_start_string="<#", comment_end_string="#>")
print(cd.from_string("<% for x in xs %>${ x }<# c #>,<% endfor %>{{ literal }}").render(xs=[1, 2]))
nl = Environment(newline_sequence="\r\n", keep_trailing_newline=True)
print(repr(nl.from_string("a\nb\n").render()))
c = Cycler("a", "b")
print(c.next(), c.next(), c.next(), c.current, end=" ")
c.reset()
print(c.current)
j = Joiner(", ")
print(repr(j()), repr(j()), Namespace(a=1).a)
ctx = env.from_string("{{ a }}").new_context({"a": 5})
print(isinstance(ctx, Context), ctx["a"], ctx.get("zz", "dflt"), sorted(ctx.get_all()), "a" in ctx)
print(jinja2.__version__, env.from_string("{{ x }}").render(x=markupsafe.Markup("<b>")))
strict = Environment(undefined=StrictUndefined)
print(strict.from_string("{{ x|default(1) }}{% if y is defined %}{{ y }}{% endif %}").render())
print(env.from_string("{{ 'é ✓ 日本' ~ '\\u00e9' }} {{ 'ß'|upper }} {{ '日本語'|length }} {{ '日本語'|reverse }}").render())
gen = env.from_string("{% for i in range(3) %}{{ i }}{% endfor %}")
print(gen.render(), gen.render(), repr(env.from_string("").render()), repr(env.from_string("  \n").render()))
print(env.from_string("{{ x.items()|list }} {{ x['a'] }} {{ x.a }} {{ x.get('b', 9) }} {{ o.val }} {{ o['val'] if false else o.double() }}").render(
    x={"a": 1}, o=type("O", (), {"val": 3, "double": lambda self: 6})()))
