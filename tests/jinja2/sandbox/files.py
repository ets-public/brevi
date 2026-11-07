# FileSystemLoader is not in Brevi's jinja2: under --sandbox the build is refused (CPython's jinja2 would need
# --allow-py jinja2); outside it the program runs with CPython's jinja2.
from jinja2 import Environment, FileSystemLoader

env = Environment(loader=FileSystemLoader("templates"))
print(env.list_templates())
