# Render a ChatML-style chat template with tools many times (the jinja2 speed gap).
import sys
import time

import jinja2

TPL = """{%- for m in messages -%}
<|im_start|>{{ m.role }}
{% if m.role == 'system' and tools %}You can call these tools:
{% for t in tools %}- {{ t.name }}({% for p in t.params %}{{ p }}{% if not loop.last %}, {% endif %}{% endfor %}): {{ t.description | trim }}
{% endfor %}{% endif %}{{ m.content | trim }}<|im_end|>
{% endfor -%}
{%- if add_generation_prompt %}<|im_start|>assistant
{% endif -%}"""

env = jinja2.Environment()
tpl = env.from_string(TPL)
tools = [{"name": "get_weather", "params": ["city", "unit"], "description": " Weather for a city "},
         {"name": "search", "params": ["query"], "description": "Search the web"},
         {"name": "calc", "params": ["expr"], "description": "Evaluate maths "}]
messages = [{"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "What's the weather in Paris?"},
            {"role": "assistant", "content": "Let me check."},
            {"role": "user", "content": "And in Rome, in celsius please?  "}]
n = int(sys.argv[1]) if len(sys.argv) > 1 else 20000
t = time.perf_counter()
total = 0
out = ""
for i in range(n):
    out = tpl.render(messages=messages, tools=tools, add_generation_prompt=i % 2 == 0)
    total += len(out)
print(out)
print(total)
print("time %.3f" % (time.perf_counter() - t), file=sys.stderr)
