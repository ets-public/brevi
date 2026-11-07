# Chat templates the way tokenizers and agent frameworks render them: ChatML with tools, a Llama 3 style
# template, a Mistral style one that raises on bad roles, and a namespace/loop-control template.
import json
from jinja2 import Environment
from jinja2.exceptions import TemplateError


def raise_exception(message):
    raise TemplateError(message)


def tojson(x, indent=None):
    return json.dumps(x, indent=indent, ensure_ascii=False)


env = Environment(trim_blocks=True, lstrip_blocks=True, extensions=["jinja2.ext.loopcontrols"])
env.filters["tojson"] = tojson
env.globals["raise_exception"] = raise_exception

CHATML = """{%- if tools %}
{{- '<|im_start|>system\\n' }}
{%- if messages[0].role == 'system' %}{{- messages[0].content + '\\n\\n' }}{%- endif %}
{{- "# Tools\\n\\nYou may call one or more functions.\\n<tools>" }}
{%- for tool in tools %}
{{- "\\n" }}{{- tool | tojson }}
{%- endfor %}
{{- "\\n</tools><|im_end|>\\n" }}
{%- elif messages[0].role == 'system' %}
{{- '<|im_start|>system\\n' + messages[0].content + '<|im_end|>\\n' }}
{%- endif %}
{%- for message in messages %}
{%- if message.role == "user" or (message.role == "system" and not loop.first) %}
{{- '<|im_start|>' + message.role + '\\n' + message.content + '<|im_end|>' + '\\n' }}
{%- elif message.role == "assistant" %}
{{- '<|im_start|>' + message.role }}
{%- if message.content %}{{- '\\n' + message.content }}{%- endif %}
{%- for tool_call in message.tool_calls | default([]) %}
{%- if tool_call.function is defined %}{%- set tool_call = tool_call.function %}{%- endif %}
{{- '\\n<tool_call>\\n{"name": "' }}{{- tool_call.name }}{{- '", "arguments": ' }}{{- tool_call.arguments | tojson }}{{- '}\\n</tool_call>' }}
{%- endfor %}
{{- '<|im_end|>\\n' }}
{%- elif message.role == "tool" %}
{%- if loop.index0 == 0 or messages[loop.index0 - 1].role != "tool" %}{{- '<|im_start|>user' }}{%- endif %}
{{- '\\n<tool_response>\\n' }}{{- message.content }}{{- '\\n</tool_response>' }}
{%- if loop.last or messages[loop.index0 + 1].role != "tool" %}{{- '<|im_end|>\\n' }}{%- endif %}
{%- endif %}
{%- endfor %}
{%- if add_generation_prompt %}{{- '<|im_start|>assistant\\n' }}{%- endif %}"""

LLAMA = """{{- bos_token }}
{%- if messages[0]['role'] == 'system' %}
    {%- set system_message = messages[0]['content']|trim %}
    {%- set messages = messages[1:] %}
{%- else %}
    {%- set system_message = "" %}
{%- endif %}
{{- "<|start_header_id|>system<|end_header_id|>\\n\\n" }}
{{- "Cutting Knowledge Date: December 2023\\n" }}
{{- "Today Date: " + date_string + "\\n\\n" }}
{{- system_message }}
{{- "<|eot_id|>" }}
{%- for message in messages %}
    {{- '<|start_header_id|>' + message['role'] + '<|end_header_id|>\\n\\n'+ message['content'] | trim + '<|eot_id|>' }}
{%- endfor %}
{%- if add_generation_prompt %}
    {{- '<|start_header_id|>assistant<|end_header_id|>\\n\\n' }}
{%- endif %}"""

MISTRAL = """{{ bos_token }}{% for message in messages %}{% if (message['role'] == 'user') != (loop.index0 % 2 == 0) %}{{ raise_exception('Conversation roles must alternate user/assistant/user/assistant/...') }}{% endif %}{% if message['role'] == 'user' %}{{ '[INST] ' + message['content'] + ' [/INST]' }}{% elif message['role'] == 'assistant' %}{{ message['content'] + eos_token}}{% else %}{{ raise_exception('Only user and assistant roles are supported!') }}{% endif %}{% endfor %}"""

AGENT = """{% set ns = namespace(found=false, turns=0, last_user='') %}
{% for m in messages %}
  {% if m.role == 'system' %}{% continue %}{% endif %}
  {% set ns.turns = ns.turns + 1 %}
  {% if m.role == 'user' %}{% set ns.last_user = m.content %}{% endif %}
  {% if 'stop' in m.content %}{% set ns.found = true %}{% break %}{% endif %}
[{{ loop.index }}/{{ loop.length }}] {{ m.role|upper }}: {{ m.content|truncate(24, true) }}
{% endfor %}
turns={{ ns.turns }} found={{ ns.found }} last_user={{ ns.last_user|tojson }}
{% for k, v in meta|dictsort %}{{ k }}={{ v }}{{ ', ' if not loop.last }}{% endfor %}
"""

messages = [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "What's the weather in Paris?"},
    {"role": "assistant", "content": "", "tool_calls": [
        {"type": "function", "function": {"name": "get_weather", "arguments": {"city": "Paris", "unit": "c"}}}]},
    {"role": "tool", "content": '{"temp": 18}'},
    {"role": "tool", "content": '{"humidity": 0.4}'},
    {"role": "assistant", "content": "It is 18°C in Paris."},
    {"role": "user", "content": "Thanks! Please stop now."},
]
tools = [{"type": "function", "function": {"name": "get_weather", "description": "Weather for a city",
          "parameters": {"type": "object", "properties": {"city": {"type": "string"}}, "required": ["city"]}}}]

t = env.from_string(CHATML)
print(t.render(messages=messages, tools=tools, add_generation_prompt=True))
print("---")
print(t.render(messages=messages[:2], tools=None, add_generation_prompt=False))
print("---")
print(env.from_string(LLAMA).render(messages=messages[:2] + [{"role": "assistant", "content": "  Sunny. "}],
                                    bos_token="<|begin_of_text|>", date_string="07 Nov 2026",
                                    add_generation_prompt=True))
print("---")
m = env.from_string(MISTRAL)
pairs = [{"role": "user", "content": "hi"}, {"role": "assistant", "content": "hello"}, {"role": "user", "content": "bye"}]
print(m.render(messages=pairs, bos_token="<s>", eos_token="</s>"))
for bad in ([{"role": "assistant", "content": "x"}], [{"role": "user", "content": "a"}, {"role": "user", "content": "b"}],
            [{"role": "system", "content": "s"}]):
    try:
        m.render(messages=bad, bos_token="<s>", eos_token="</s>")
    except TemplateError as e:
        print("TemplateError:", e)
print("---")
print(env.from_string(AGENT).render(messages=messages, meta={"model": "m1", "temperature": 0.2, "id": 7}))
