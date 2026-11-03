# Makes src/dynpy/py/argparse.py, Brevi's argparse for native builds, from CPython 3.13's Lib/argparse.py:
#   python3 tools/mkargparse.py /usr/lib/python3.13/argparse.py src/dynpy/py/argparse.py
# The edits are the ones the dynamic build needs; parsing, errors and help text stay CPython's.
import re, sys
s = open(sys.argv[1]).read()
def rep(old, new, count=1):
    global s
    assert s.count(old) >= 1, old
    s = s.replace(old, new) if count == 0 else s.replace(old, new, count)
rep("from gettext import gettext as _, ngettext", "def _(message):\n    return message\n\n\ndef ngettext(singular, plural, n):\n    return singular if n == 1 else plural")
rep("    import copy\n    return copy.copy(items)", "    if type(items) is list:\n        return items[:]\n    return items")
a = s.index("    def _iter_indented_subactions(self, action):"); b = s.index("    def _split_lines(")
s = s[:a] + """    def _iter_indented_subactions(self, action):
        get_subactions = getattr(action, '_get_subactions', None)
        if get_subactions is not None:
            self._indent()
            yield from get_subactions()
            self._dedent()

""" + s[b:]
rep("            import shutil\n            width = shutil.get_terminal_size().columns", "            import _brevi_term\n            width = _brevi_term.columns()")
# warnings (stderr only): dropped
a = s.index("        for field_name in ('type', 'choices', 'metavar'):\n            if locals()"); b = s.index("        if type is _deprecated_default:")
s = s[:a] + s[b:]
s = re.sub(r"        import warnings\n        warnings\.warn\(\n(?:            .*\n)*?        \)\n", "", s)
rep("            sup = super(_SubParsersAction._ChoicesPseudoAction, self)\n            sup.__init__(", "            super(_SubParsersAction._ChoicesPseudoAction, self).__init__(")
rep("        super_init = super(_ArgumentGroup, self).__init__\n        super_init(", "        super().__init__(")
rep("        superinit = super(ArgumentParser, self).__init__\n        superinit(", "        super().__init__(")
s = re.sub(r"super\(\w+, self\)", "super()", s)
# Files: FileType and @file arguments open files; a program using them runs in CPython (dynpy/mod.rs), so the
# copy has neither and opens nothing.
a = s.index("# ==============\n# Type classes\n"); b = s.index("# ===========================\n# Optional and Positional Parsing")
s = s[:a] + s[b:]
rep("    'FileType',\n", "")
a = s.index("                try:\n                    with open(arg_string[1:],"); b = s.index("        # return the modified argument list\n        return new_arg_strings")
s = s[:a] + "                raise ArgumentError(None, 'fromfile_prefix_chars is not in the native build')\n\n" + s[b:]
rep("        if type_func is FileType:\n            raise ValueError('%r is a FileType class object, instance of it'\n                             ' must be passed' % (type_func,))\n\n", "")
HEADER = """# argparse for native builds (src/dynpy/mod.rs): CPython 3.13's Lib/argparse.py, made by tools/mkargparse.py.
# Copyright (c) 2001-2024 Python Software Foundation; All Rights Reserved. PSF License Version 2:
# src/dynpy/py/LICENSE-PSF.txt. Changes from CPython's file: no gettext (messages in English), no warnings
# (they went to stderr only), no FileType or @file arguments (a program using them runs in CPython), the
# help width from _brevi_term (as shutil.get_terminal_size), and super() and one generator written the way
# Brevi's dynamic build takes them.
"""
open(sys.argv[2], "w").write(HEADER + s)
