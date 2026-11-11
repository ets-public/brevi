# _string for native builds (src/dynpy/mod.rs): CPython's _string.formatter_parser and
# _string.formatter_field_name_split (C in CPython), written in Python for Brevi's dynamic build.

def formatter_parser(format_string):
    """Yield (literal_text, field_name, format_spec, conversion) tuples for format_string."""
    if not isinstance(format_string, str):
        raise TypeError("expected str, got " + type(format_string).__name__)
    i = 0
    n = len(format_string)
    literal = []
    while i < n:
        c = format_string[i]
        if c == '{':
            if i + 1 < n and format_string[i + 1] == '{':
                literal.append('{')
                i += 2
                yield ''.join(literal), None, None, None
                literal = []
                continue
            if i + 1 >= n:
                raise ValueError("Single '{' encountered in format string")
            i += 1
            field = []
            in_bracket = False
            field_name = None
            spec = None
            conversion = None
            while True:
                if i >= n:
                    raise ValueError("expected '}' before end of string")
                c = format_string[i]
                if in_bracket:
                    field.append(c)
                    if c == ']':
                        in_bracket = False
                    i += 1
                    continue
                if c == '[':
                    in_bracket = True
                    field.append(c)
                    i += 1
                elif c == '}':
                    field_name = ''.join(field)
                    spec = ''
                    conversion = None
                    i += 1
                    break
                elif c == '!':
                    i += 1
                    if i >= n:
                        raise ValueError("end of string while looking for conversion specifier")
                    conversion = format_string[i]
                    i += 1
                    if i >= n:
                        raise ValueError("unmatched '{' in format spec")
                    if format_string[i] == '}':
                        field_name = ''.join(field)
                        spec = ''
                        i += 1
                        break
                    if format_string[i] != ':':
                        raise ValueError("expected ':' after conversion specifier")
                    i += 1
                    spec, i = _format_spec(format_string, i)
                    field_name = ''.join(field)
                    break
                elif c == ':':
                    i += 1
                    conversion = None
                    spec, i = _format_spec(format_string, i)
                    field_name = ''.join(field)
                    break
                elif c == '{':
                    raise ValueError("unexpected '{' in field name")
                else:
                    field.append(c)
                    i += 1
            yield ''.join(literal), field_name, spec, conversion
            literal = []
        elif c == '}':
            if i + 1 < n and format_string[i + 1] == '}':
                literal.append('}')
                i += 2
                yield ''.join(literal), None, None, None
                literal = []
                continue
            raise ValueError("Single '}' encountered in format string")
        else:
            literal.append(c)
            i += 1
    if literal:
        yield ''.join(literal), None, None, None


def _format_spec(s, i):
    # The format spec after ':' up to the matching '}', with nested braces kept
    # as written (Formatter._vformat parses them at the next level).
    depth = 0
    start = i
    while i < len(s):
        c = s[i]
        if c == '{':
            depth += 1
        elif c == '}':
            if depth == 0:
                return s[start:i], i + 1
            depth -= 1
        i += 1
    raise ValueError("unmatched '{' in format spec")


def formatter_field_name_split(field_name):
    """Split field_name into (first, rest); rest is an iterator of (is_attr, value)."""
    if not isinstance(field_name, str):
        raise TypeError("expected str, got " + type(field_name).__name__)
    n = len(field_name)
    i = 0
    while i < n and field_name[i] != '.' and field_name[i] != '[':
        i += 1
    first = field_name[:i]
    if first.isdecimal():
        first = int(first)

    def rest():
        j = i
        while j < n:
            if field_name[j] == '.':
                j += 1
                start = j
                while j < n and field_name[j] != '.' and field_name[j] != '[':
                    j += 1
                if j == start:
                    raise ValueError('Empty attribute in format string')
                yield True, field_name[start:j]
            else:
                j += 1
                end = field_name.find(']', j)
                if end < 0:
                    raise ValueError("Missing ']' in format string")
                item = field_name[j:end]
                if not item:
                    raise ValueError('Empty attribute in format string')
                if item.isdecimal():
                    item = int(item)
                yield False, item
                j = end + 1
                if j < n and field_name[j] != '.' and field_name[j] != '[':
                    raise ValueError("Only '.' or '[' may follow ']' in format field specifier")

    return first, rest()
