class Font:
    def __init__(self, *a, **kw):
        self.opts = kw

    def configure(self, **kw):
        self.opts.update(kw)

    config = configure

    def measure(self, text):
        return 7 * len(str(text))


BOLD = "bold"; ITALIC = "italic"; NORMAL = "normal"
