"""Builds the Daytona snapshot `brevi` from templates/image/Dockerfile. Run it once, and again (after
deleting the old snapshot) to pick up a new Brevi release.

    pip install daytona && export DAYTONA_API_KEY=... && python templates/daytona/build_snapshot.py
"""
import os

from daytona import CreateSnapshotParams, Daytona, Image, Resources

here = os.path.dirname(os.path.abspath(__file__))
# BREVI_IMAGE=ghcr.io/ets-public/brevi-sandbox:latest starts from the published image (from v0.27.0) instead.
published = os.environ.get("BREVI_IMAGE")
image = Image.base(published) if published else Image.from_dockerfile(os.path.join(here, "..", "image", "Dockerfile"))
# rustc wants memory when it links a program: give each sandbox 2 vCPUs and 2 GB.
snap = Daytona().snapshot.create(
    CreateSnapshotParams(name=os.environ.get("BREVI_DAYTONA_SNAPSHOT", "brevi"), image=image,
                         resources=Resources(cpu=2, memory=2), entrypoint=["sleep", "infinity"]),
    on_logs=print,
)
print("built snapshot", snap.name)
