# Autoescaping, Markup and markupsafe as HTML prompt and report code uses them.
from jinja2 import Environment, DictLoader, select_autoescape
from markupsafe import Markup, escape, soft_str

env = Environment(loader=DictLoader({
    "page.html": "<h1>{{ title }}</h1>{{ body }}{{ safe|safe }}{% autoescape false %}{{ title }}{% endautoescape %}{% include 'p.txt' %}",
    "p.txt": "[{{ title }}]",
    "m.html": "{% macro b(x) %}<b>{{ x }}</b>{% endmacro %}{{ b(title) }}{{ b(title)|upper }}{% set s %}<i>{{ title }}</i>{% endset %}{{ s }}{{ s|length }}",
}), autoescape=select_autoescape(["html"]))
kw = {"title": "Tom & <Jerry>", "body": Markup("<p>ok</p>"), "safe": "<em>raw</em>"}
print(env.get_template("page.html").render(**kw))
print(env.get_template("m.html").render(**kw))
e2 = Environment(autoescape=True)
for t in ["{{ x }}", "{{ x|e }}", "{{ x|safe|e }}", "{{ x ~ '<' }}", "{{ x|replace('&', '+') }}", "{{ [x]|join(',') }}",
          "{{ x|safe ~ '<' }}", "{{ x|striptags }}", "{{ '%s<'|format(x) }}", "{{ x|truncate(5) }}", "{{ x|urlize }}",
          "{{ {'a': x}|tojson }}", "{{ x|indent(1) }}", "{{ x|title }}", "{{ x|wordwrap(3) }}", "{{ (x|safe)[0:3] }}",
          "{{ x|forceescape|safe }}", "{{ x|string }}", "{{ x is escaped }} {{ x|e is escaped }}", "{{ x|center(16) }}"]:
    print(repr(e2.from_string(t).render(x="a<b>&'\"c")))
m = Markup("<b>%s</b>") % "<i>"
print(repr(m), m.striptags(), Markup("&lt;x&gt; &amp; &#39;y&#39; &#x41; &copy;").unescape())
print(repr(escape("<a href='x'>")), repr(escape(Markup("<ok>"))), repr(escape(5)), repr(soft_str(Markup("x"))))
print(repr(Markup("a") + "<"), repr("<" + Markup("a")), repr(Markup(", ").join(["<", Markup("<b>")])), repr(Markup("x") * 2))
print(repr(Markup("<p>a b</p>").split()), repr(Markup("<b>{0}</b> {n}").format("<", n="&")), repr(Markup("%(a)s") % {"a": "<"}))
class H:
    def __html__(self):
        return "<h>html</h>"
print(repr(escape(H())), env.from_string("{{ h }}").render(h=H()), e2.from_string("{{ h }}").render(h=H()))
print(e2.from_string("{% autoescape false %}{{ x }}{% endautoescape %}{{ x }}").render(x="<"))
print(Environment(autoescape=lambda n: n is not None and n.endswith(".x")).from_string("{{ '<' }}").render())
