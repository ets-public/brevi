# Classes built on a library's classes run in CPython whole; the rest of the program stays native.
from typing import List, Optional
from brlib import Model, LibError


class Author(Model):
    name: str
    handle: Optional[str] = None


class Post(Model):
    title: str
    tags: List[str] = []
    author: Author = None

    def short(self):
        return f"{self.title} ({len(self.tags)} tags)"


class Pinned(Post):
    order: int = 0


def build(rows):
    out = []
    for r in rows:
        try:
            out.append(Post(**r))
        except LibError as e:
            print("skip:", e)
    return out


posts = build([{"title": "a", "tags": ["x", "y"]}, {"tags": []}, {"title": "b", "author": Author(name="Ann")}])
print(len(posts), [p.short() for p in posts], posts[1].author.name, posts[0].author)
p = Pinned(title="top", order=1)
print(p, isinstance(p, Post), isinstance(p, Model), p.dump())
counts = {}
for p in posts:
    for t in p.tags:
        counts[t] = counts.get(t, 0) + 1
print(counts, sorted(p.title for p in posts))
