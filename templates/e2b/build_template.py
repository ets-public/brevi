"""Builds the E2B template `brevi` from templates/image/Dockerfile. Run it once, and again to pick up a new
Brevi release (pass --fresh to skip E2B's layer cache).

    pip install e2b && export E2B_API_KEY=... && python templates/e2b/build_template.py
"""
import os, sys

from e2b import Template, default_build_logger

here = os.path.dirname(os.path.abspath(__file__))
dockerfile = open(os.path.join(here, "..", "image", "Dockerfile")).read()
base = Template().skip_cache() if "--fresh" in sys.argv else Template()
# BREVI_IMAGE=ghcr.io/ets-public/brevi-sandbox:latest starts from the published image (from v0.27.0) instead.
published = os.environ.get("BREVI_IMAGE")
template = base.from_image(published) if published else base.from_dockerfile(dockerfile)
# rustc wants memory when it links a program: give each sandbox 2 vCPUs and 2 GB.
info = Template.build(template, os.environ.get("BREVI_E2B_TEMPLATE", "brevi"), cpu_count=2, memory_mb=2048,
                      on_build_logs=default_build_logger())
print("built template", info.name, info.template_id)
