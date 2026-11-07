# Brevi's jinja2.utils (see jinja2.py).
from jinja2 import Cycler, Joiner, Namespace, clear_caches, generate_lorem_ipsum, htmlsafe_json_dumps, is_undefined, missing, object_type_repr, pass_context, pass_environment, pass_eval_context, select_autoescape, url_quote, urlize


def concat(seq):
    return "".join(seq)
