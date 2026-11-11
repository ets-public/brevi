# textwrap for native builds (src/dynpy/mod.rs): wrap, fill, shorten, dedent, indent and TextWrapper, with
# CPython's rules for where lines break (its word-splitting pattern is written out by hand below).

_WS = "\t\n\x0b\x0c\r "


def _w(c):
    return c.isalnum() or c == "_"


def _lt(c):
    return c.isalpha() or c == "_"


def _wp(c):
    return _w(c) or c in "!\"'&.,?"


def _dashes_then_word(t, j):
    # -{2,} followed by a word character, starting at j
    n = len(t)
    k = j
    while k < n and t[k] == "-":
        k += 1
    return k - j >= 2 and k < n and _w(t[k])


def _chunks(t, hyphens):
    out = []
    n = len(t)
    i = 0
    while i < n:
        c = t[i]
        if c in _WS:
            j = i
            while j < n and t[j] in _WS:
                j += 1
            out.append(t[i:j])
            i = j
            continue
        if not hyphens:
            j = i
            while j < n and t[j] not in _WS:
                j += 1
            out.append(t[i:j])
            i = j
            continue
        if c == "-" and i > 0 and _wp(t[i - 1]) and _dashes_then_word(t, i):
            j = i
            while t[j] == "-":
                j += 1
            out.append(t[i:j])
            i = j
            continue
        j = i + 1
        while True:
            if j < n and t[j] == "-":
                behind = (j >= 2 and _lt(t[j - 2]) and _lt(t[j - 1])) or (j >= 3 and _lt(t[j - 3]) and t[j - 2] == "-" and _lt(t[j - 1]))
                ahead = j + 1 < n and _lt(t[j + 1]) and ((j + 2 < n and _lt(t[j + 2])) or (j + 3 < n and t[j + 2] == "-" and _lt(t[j + 3])))
                if behind and ahead:
                    out.append(t[i:j + 1])
                    i = j + 1
                    break
            if j >= n or t[j] in _WS:
                out.append(t[i:j])
                i = j
                break
            if _wp(t[j - 1]) and _dashes_then_word(t, j):
                out.append(t[i:j])
                i = j
                break
            j += 1
    return out


def _sentence_end(chunk):
    # Equivalent of CPython's _sentence_end_re: [a-z][.!?]["']?\Z
    n = len(chunk)
    if n >= 2 and "a" <= chunk[-2] <= "z" and chunk[-1] in ".!?":
        return True
    return n >= 3 and "a" <= chunk[-3] <= "z" and chunk[-2] in ".!?" and chunk[-1] in "\"'"


class TextWrapper:
    def __init__(self, width=70, initial_indent="", subsequent_indent="", expand_tabs=True, replace_whitespace=True, fix_sentence_endings=False, break_long_words=True, drop_whitespace=True, break_on_hyphens=True, tabsize=8, max_lines=None, placeholder=" [...]"):
        self.width = width
        self.initial_indent = initial_indent
        self.subsequent_indent = subsequent_indent
        self.expand_tabs = expand_tabs
        self.replace_whitespace = replace_whitespace
        self.fix_sentence_endings = fix_sentence_endings
        self.break_long_words = break_long_words
        self.drop_whitespace = drop_whitespace
        self.break_on_hyphens = break_on_hyphens
        self.tabsize = tabsize
        self.max_lines = max_lines
        self.placeholder = placeholder

    def _munge_whitespace(self, text):
        if self.expand_tabs:
            text = text.expandtabs(self.tabsize)
        if self.replace_whitespace:
            out = []
            for c in text:
                out.append(" " if c in _WS else c)
            text = "".join(out)
        return text

    def _split(self, text):
        return [c for c in _chunks(text, self.break_on_hyphens) if c]

    def _fix_sentence_endings(self, chunks):
        i = 0
        while i < len(chunks) - 1:
            if chunks[i + 1] == " " and _sentence_end(chunks[i]):
                chunks[i + 1] = "  "
                i += 2
            else:
                i += 1

    def _handle_long_word(self, chunks, cur_line, cur_len, width):
        if width < 1:
            space_left = 1
        else:
            space_left = width - cur_len
        if self.break_long_words and space_left > 0:
            end = space_left
            chunk = chunks[-1]
            if self.break_on_hyphens and len(chunk) > space_left:
                hyphen = chunk.rfind("-", 0, space_left)
                if hyphen > 0 and any(c != "-" for c in chunk[:hyphen]):
                    end = hyphen + 1
            cur_line.append(chunk[:end])
            chunks[-1] = chunk[end:]
        elif not cur_line:
            cur_line.append(chunks.pop())

    def _wrap_chunks(self, chunks):
        lines = []
        if self.width <= 0:
            raise ValueError("invalid width " + repr(self.width) + " (must be > 0)")
        if self.max_lines is not None:
            if self.max_lines > 1:
                indent = self.subsequent_indent
            else:
                indent = self.initial_indent
            if len(indent) + len(self.placeholder.lstrip()) > self.width:
                raise ValueError("placeholder too large for max width")
        chunks.reverse()
        while chunks:
            cur_line = []
            cur_len = 0
            if lines:
                indent = self.subsequent_indent
            else:
                indent = self.initial_indent
            width = self.width - len(indent)
            if self.drop_whitespace and chunks[-1].strip() == "" and lines:
                del chunks[-1]
            while chunks:
                l = len(chunks[-1])
                if cur_len + l <= width:
                    cur_line.append(chunks.pop())
                    cur_len += l
                else:
                    break
            if chunks and len(chunks[-1]) > width:
                self._handle_long_word(chunks, cur_line, cur_len, width)
                cur_len = sum([len(c) for c in cur_line])
            if self.drop_whitespace and cur_line and cur_line[-1].strip() == "":
                cur_len -= len(cur_line[-1])
                del cur_line[-1]
            if cur_line:
                if self.max_lines is None or len(lines) + 1 < self.max_lines or ((not chunks or (self.drop_whitespace and len(chunks) == 1 and not chunks[0].strip())) and cur_len <= width):
                    lines.append(indent + "".join(cur_line))
                else:
                    done = False
                    while cur_line:
                        if cur_line[-1].strip() and cur_len + len(self.placeholder) <= width:
                            cur_line.append(self.placeholder)
                            lines.append(indent + "".join(cur_line))
                            done = True
                            break
                        cur_len -= len(cur_line[-1])
                        del cur_line[-1]
                    if not done:
                        if lines:
                            prev_line = lines[-1].rstrip()
                            if len(prev_line) + len(self.placeholder) <= self.width:
                                lines[-1] = prev_line + self.placeholder
                                break
                        lines.append(indent + self.placeholder.lstrip())
                    break
        return lines

    def wrap(self, text):
        chunks = self._split(self._munge_whitespace(text))
        if self.fix_sentence_endings:
            self._fix_sentence_endings(chunks)
        return self._wrap_chunks(chunks)

    def fill(self, text):
        return "\n".join(self.wrap(text))


def wrap(text, width=70, **kwargs):
    return TextWrapper(width=width, **kwargs).wrap(text)


def fill(text, width=70, **kwargs):
    return TextWrapper(width=width, **kwargs).fill(text)


def shorten(text, width, **kwargs):
    w = TextWrapper(width=width, max_lines=1, **kwargs)
    return w.fill(" ".join(text.strip().split()))


def dedent(text):
    lines = text.split("\n")
    for k in range(len(lines)):
        l = lines[k]
        if l and l.strip(" \t") == "":
            lines[k] = ""
    margin = None
    for l in lines:
        s = l.lstrip(" \t")
        if not s:
            continue
        ind = l[:len(l) - len(s)]
        if margin is None:
            margin = ind
        elif ind.startswith(margin):
            pass
        elif margin.startswith(ind):
            margin = ind
        else:
            i = 0
            while i < len(margin) and i < len(ind) and margin[i] == ind[i]:
                i += 1
            margin = margin[:i]
    if margin:
        lines = [l[len(margin):] if l.startswith(margin) else l for l in lines]
    return "\n".join(lines)


def indent(text, prefix, predicate=None):
    out = []
    for line in text.splitlines(True):
        if (predicate(line) if predicate is not None else line.strip()):
            out.append(prefix + line)
        else:
            out.append(line)
    return "".join(out)
