"""Writes src/pynames/*.txt, the Python names src/pydiag.rs trusts: builtins, every public method or attribute of
the built-in types and of the common standard-library modules and their classes, and the modules covered.
usage: python3 tools/py_names.py   (run with the newest CPython; a name it lacks is only ever less certain)"""
import builtins, importlib, inspect
names=set()
for t in [str,bytes,bytearray,list,dict,set,frozenset,tuple,int,float,complex,bool,range,memoryview,object,type,slice,BaseException]:
    names|=set(dir(t))
mods="re collections datetime decimal fractions itertools functools heapq math cmath json csv io pathlib random string sys os os.path time typing dataclasses enum bisect copy statistics operator textwrap array queue threading subprocess argparse logging unittest struct hashlib base64 uuid urllib.parse shutil glob tempfile pprint calendar zlib gzip pickle sqlite3 secrets abc contextlib numbers types weakref unicodedata difflib fnmatch locale signal socket select asyncio concurrent.futures multiprocessing traceback warnings inspect platform getpass shlex html html.parser xml.etree.ElementTree tkinter".split()
for m in mods:
    try: mod=importlib.import_module(m)
    except Exception: continue
    for n in dir(mod):
        if n.startswith('_'): continue
        names.add(n)
        v=getattr(mod,n,None)
        if isinstance(v,type):
            try: names|=set(dir(v))
            except Exception: pass
# file objects, match objects, generators
import io as _io
names|=set(dir(_io.TextIOWrapper)); names|=set(dir(_io.BufferedReader))
import re
names|=set(dir(re.match('a','a'))); names|=set(dir(re.compile('a')))
def g(): yield 1
names|=set(dir(g()))
names={n for n in names if not n.startswith('_')}
import os
D = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'src', 'pynames')
open(os.path.join(D, 'methods.txt'),'w').write(' '.join(sorted(names)))
b=sorted(n for n in dir(builtins))
open(os.path.join(D, 'builtins.txt'),'w').write(' '.join(b))
open(os.path.join(D, 'mods.txt'),'w').write(' '.join(mods))
core=set()
for t in [str,list,dict,set,int,float,tuple,bytes]: core|=set(dir(t))
open(os.path.join(D, 'core.txt'),'w').write(' '.join(sorted(n for n in core if not n.startswith('_')))+'\n')
