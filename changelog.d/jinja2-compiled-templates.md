### Changed
- Brevi's jinja2 now compiles each template into closures on its first render instead of walking the template's tree on every render, makes no loop object for a `{% for %}` whose body never uses `loop`, and copies the globals into each render's context in one pass. A chat template with tools renders about 2x faster (2.2x CPython's time, down from 4.8x).

### Fixed
- In Brevi's jinja2, a template included inside a `{% for %}` loop could see the loop's `loop` variable; in Jinja it cannot.
