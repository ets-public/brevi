# Brevi's jinja2.runtime (see jinja2.py).
from jinja2 import BlockReference, ChainableUndefined, Context, DebugUndefined, EvalContext, LoopContext, Macro, Markup, Namespace, StrictUndefined, TemplateNotFound, TemplateReference, TemplateRuntimeError, Undefined, UndefinedError, escape, markup_join, missing, new_context, object_type_repr, pass_eval_context, soft_str, str_join


def identity(x):
    return x
