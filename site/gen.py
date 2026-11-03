#!/usr/bin/env python3
"""Builds the HTML pages of brevi-lang.org and brevi.run from site/ (no packages needed).

  python3 site/gen.py OUT OUT_RUN VERSION GUIDE_TOKENS PUBLIC [FULL_VERSION]

- The home page (site/index.html) and the brevi.run page (site/run/index.html) are templates: __HEADER__,
  __FOOTER__, __VERSION__ (major.minor), __FULLVERSION__ (as `brevi --version` prints it), __GUIDE_TOKENS__ and the public/private blocks below are filled in.
- Docs pages come from site/docs/*.md, plus four pages made straight from the repo's own files
  (docs/ARCHITECTURE.md, BREVI.md, REFERENCE.md, LIBRARIES.md), so they cannot drift from it. Each page is
  written as OUT/docs/NAME.html and, for agents, as OUT/docs/NAME.md.
- `<!--public-->...<!--/public-->` is kept only once the GitHub repo is public (PUBLIC=1) and
  `<!--private-->...<!--/private-->` only before. Links written `repo:PATH` point at the file on GitHub when
  public and become plain text before, since a private repo's links 404 for visitors.
- Mermaid diagrams are drawn ahead of time into site/docs/img/mmd-HASH.svg by `node site/tools/diagrams.mjs`
  (needs playwright and mermaid); a diagram without its SVG is shown as its source, with a warning.
"""
import hashlib, html, json, os, re, sys

S = os.path.dirname(os.path.abspath(__file__))
R = os.path.dirname(S)
GITHUB = "https://github.com/ets-public/brevi"

# Docs navigation: (group, [(slug, title, source)]). Source "x.md" is in site/docs, "repo:X" a repo file.
NAV = [
    ("Start", [
        ("index", "Overview", "index.md"),
        ("install", "Install", "install.md"),
        ("usage", "Using Brevi", "usage.md"),
    ]),
    ("For AI agents", [
        ("agents", "Agents, MCP and skills", "agents.md"),
    ]),
    ("How it works", [
        ("architecture", "Architecture", "repo:docs/ARCHITECTURE.md"),
        ("compatibility", "What works, and limits", "compatibility.md"),
        ("benchmarks", "Benchmarks", "benchmarks.md"),
    ]),
    ("Brevi language (.bv)", [
        ("language", "Language guide", "repo:BREVI.md"),
        ("reference", "Reference", "repo:REFERENCE.md"),
        ("libraries", "Libraries", "repo:LIBRARIES.md"),
    ]),
    ("Project", [
        ("faq", "FAQ", "faq.md"),
        ("roadmap", "Roadmap", "roadmap.md"),
        ("changelog", "Changelog", "repo:CHANGELOG.md"),
        ("contributing", "Contributing and license", "contributing.md"),
    ]),
]
# Repo files written one statement per line (the model guides): each line is its own paragraph.
LINE_PER_PARA = {"repo:BREVI.md", "repo:REFERENCE.md", "repo:LIBRARIES.md"}
INTROS = {
    "repo:BREVI.md": "Brevi's own syntax, for `.bv` files. You do not need it to build Python. This is the guide built into the compiler (`brevi guide`); it is written for models, so it is dense on purpose.",
    "repo:REFERENCE.md": "Details kept out of the core guide. `brevi guide WORD` prints only the lines about one name.",
    "repo:LIBRARIES.md": "Other `.bv` files, Rust crates and Python code from Brevi programs, and Python libraries from Python files (`brevi guide Libraries`).",
    "repo:CHANGELOG.md": "What changed in each release, newest first, with what is on main for the next one under Unreleased. Built from [CHANGELOG.md](repo:CHANGELOG.md) in the repository; each release's section is also its GitHub release notes.",
    "repo:docs/ARCHITECTURE.md": "This page is built from [docs/ARCHITECTURE.md](repo:docs/ARCHITECTURE.md) in the repository, so it always describes the code that shipped.",
}

warnings = []
FULLVERSION = ""


def blocks_for(text, public):
    keep, drop = ("public", "private") if public else ("private", "public")
    text = re.sub(r"<!--%s-->(.*?)<!--/%s-->" % (drop, drop), "", text, flags=re.S)
    return re.sub(r"<!--/?%s-->" % keep, "", text)


def slug(t):
    t = re.sub(r"<[^>]+>", "", t)
    t = re.sub(r"[^\w\- ]", "", t.lower()).strip()
    return re.sub(r"[\s]+", "-", t)


# ---------------------------------------------------------------- Markdown (the subset these files use)

def inline(t, ctx):
    codes = []

    def keep_code(m):
        codes.append("<code>%s</code>" % html.escape(m.group(2)))
        return "\x00%d\x00" % (len(codes) - 1)

    t = re.sub(r"(`+)(.+?)\1", keep_code, t)
    raw = []

    def keep_raw(m):  # inline HTML written in the source (<br>, <kbd>)
        raw.append(m.group(0))
        return "\x01%d\x01" % (len(raw) - 1)

    t = re.sub(r"</?(br|kbd|sup|sub|span|em|strong|a)\b[^>]*>", keep_raw, t)
    t = html.escape(t, quote=False)

    def link(m):
        img, label, url = m.group(1), m.group(2), m.group(3).strip()
        if img:
            return '<img src="%s" alt="%s" loading="lazy">' % (html.escape(url), label)
        url = fix_url(url, ctx)
        if url is None:
            return label
        ext = url.startswith("http") and not url.startswith(("https://brevi-lang.org", "https://brevi.run"))
        return '<a href="%s"%s>%s</a>' % (html.escape(url), ' rel="noopener"' if ext else "", label)

    t = re.sub(r"(!?)\[([^\]]+)\]\(([^)\s]+)\)", lambda m: link(m) if True else "", t)
    t = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", t)
    t = re.sub(r"(?<![\w*])\*(?!\s)(.+?)(?<!\s)\*(?![\w*])", r"<em>\1</em>", t)
    t = re.sub(r"(?<![\w])_(?!\s)([^_]+?)(?<!\s)_(?![\w])", r"<em>\1</em>", t)
    t = re.sub(r"\x01(\d+)\x01", lambda m: raw[int(m.group(1))], t)
    return re.sub(r"\x00(\d+)\x00", lambda m: codes[int(m.group(1))], t)


def fix_url(url, ctx):
    if url.startswith("repo:"):
        path = url[5:]
        if not ctx["public"]:
            return None
        kind = "tree" if path.endswith("/") or "." not in path.split("/")[-1] else "blob"
        return "%s/%s/main/%s" % (GITHUB, kind, path)
    if url.startswith(("http:", "https:", "mailto:", "#", "/")):
        if url.startswith(GITHUB) and not ctx["public"]:
            return None
        return url
    # A relative link inside a repo file (docs/ARCHITECTURE.md -> ../bench/README.md).
    if ctx.get("repo_dir") is not None:
        path = os.path.normpath(os.path.join(ctx["repo_dir"], url.split("#")[0]))
        for slug_, src in ctx["repo_pages"].items():
            if src == "repo:" + path:
                return "/docs/" + slug_ + ("#" + url.split("#")[1] if "#" in url else "")
        return fix_url("repo:" + path, ctx)
    return url


def table(rows, ctx):
    def cells(r):
        r = r.strip()
        r = r[1:] if r.startswith("|") else r
        r = r[:-1] if r.endswith("|") else r
        return [c.strip() for c in re.split(r"(?<!\\)\|", r)]

    head, align, body = cells(rows[0]), cells(rows[1]), [cells(r) for r in rows[2:]]
    al = ["right" if a.endswith(":") and not a.startswith(":") else "" for a in align]

    def td(tag, c, i):
        cls = ' class="n"' if i < len(al) and al[i] == "right" else ""
        return "<%s%s>%s</%s>" % (tag, cls, inline(c.replace("\\|", "|"), ctx), tag)

    out = ['<div class="table-wrap"><table><thead><tr>']
    out += [td("th", c, i) for i, c in enumerate(head)]
    out.append("</tr></thead><tbody>")
    for r in body:
        out.append("<tr>" + "".join(td("td", c, i) for i, c in enumerate(r)) + "</tr>")
    out.append("</tbody></table></div>")
    return "".join(out)


def svg_size(path):
    """Width and height of an SVG from its viewBox, in CSS pixels (its natural size)."""
    m = re.search(r'viewBox="[\d.\-]+ [\d.\-]+ ([\d.]+) ([\d.]+)"', open(path).read(2000))
    return (round(float(m.group(1))), round(float(m.group(2)))) if m else (800, 400)


def svg_natural(text):
    """Give a Mermaid SVG its natural size (mermaid writes width="100%" and a max-width style), so that
    opened on its own in a tab it shows at 100% and the browser's zoom works, instead of filling the tab."""
    m = re.match(r"(<svg\b[^>]*>)", text)
    if not m:
        return text
    root = m.group(1)
    vb = re.search(r'viewBox="[\d.\-]+ [\d.\-]+ ([\d.]+) ([\d.]+)"', root)
    if not vb:
        return text
    r = re.sub(r'\s(width|height)="[^"]*"', "", root)
    r = re.sub(r'\sstyle="max-width:[^"]*"', "", r)
    r = r[:-1] + ' width="%s" height="%s">' % (vb.group(1), vb.group(2))
    return r + text[len(root):]


def mermaid(src, ctx=None):
    h = hashlib.sha1(src.strip().encode()).hexdigest()[:12]
    svg = os.path.join(S, "docs", "img", "mmd-%s.svg" % h)
    if os.path.exists(svg):
        w, ht = svg_size(svg)
        where = (ctx or {}).get("section") or "this page"
        alt = html.escape("Diagram: %s" % where, quote=True)
        url = "/docs/img/mmd-%s.svg" % h
        # A link to the SVG itself, so the diagram opens full size even without JavaScript; the docs
        # page's script turns the click into a zoomable view (site/gen.py ZOOM_JS).
        return h, ('<figure class="diagram"><a class="zoom" href="%s" target="_blank" rel="noopener" data-w="%d" data-h="%d" '
                   'aria-label="%s, open full size"><img src="%s" alt="%s" width="%d" height="%d" loading="lazy"></a>'
                   '<figcaption>Click the diagram to enlarge it</figcaption></figure>' % (url, w, ht, alt, url, alt, w, ht))
    warnings.append("diagram mmd-%s.svg missing: run node site/tools/diagrams.mjs" % h)
    return h, '<pre class="code"><code>%s</code></pre>' % html.escape(src)


def md(text, ctx, line_per_para=False):
    """Markdown to HTML. Returns (html, [(level, id, title)])."""
    lines = text.split("\n")
    out, toc, i = [], [], 0
    para, ids = [], set()

    def flush():
        if para:
            if line_per_para:
                out.extend("<p>%s</p>" % inline(l, ctx) for l in para)
            else:
                out.append("<p>%s</p>" % inline(" ".join(para), ctx))
            para.clear()

    def list_block(start):
        # Bulleted or numbered list, with nested lists by indentation and continued lines.
        items, j = [], start
        base = len(lines[j]) - len(lines[j].lstrip())
        ordered = bool(re.match(r"\s*\d+\.\s", lines[j]))
        while j < len(lines):
            l = lines[j]
            ind = len(l) - len(l.lstrip())
            m = re.match(r"\s*(?:[-*]|\d+\.)\s+(.*)", l)
            if m and ind == base:
                items.append([m.group(1)])
            elif l.strip() and ind > base and items:
                items[-1].append(l[base + 2:] if ind >= base + 2 else l.strip())
            elif not l.strip() and j + 1 < len(lines) and re.match(r"\s*(?:[-*]|\d+\.)\s", lines[j + 1]) and len(lines[j + 1]) - len(lines[j + 1].lstrip()) >= base:
                pass
            else:
                break
            j += 1
        tag = "ol" if ordered else "ul"
        html_items = []
        for it in items:
            first, rest = [it[0]], []
            k = 1
            while k < len(it) and not re.match(r"\s*(?:[-*]|\d+\.)\s", it[k]):
                first.append(it[k].strip())
                k += 1
            rest = it[k:]
            inner = inline(" ".join(first), ctx)
            if rest:
                inner += md("\n".join(rest), ctx)[0]
            html_items.append("<li>%s</li>" % inner)
        return "<%s>%s</%s>" % (tag, "".join(html_items), tag), j

    while i < len(lines):
        l = lines[i]
        s = l.strip()
        if s.startswith("```"):
            flush()
            lang = s[3:].strip()
            j = i + 1
            while j < len(lines) and not lines[j].strip().startswith("```"):
                j += 1
            code = "\n".join(lines[i + 1:j])
            if lang == "mermaid":
                out.append(mermaid(code, ctx)[1])
            else:
                cls = ' data-lang="%s"' % html.escape(lang) if lang else ""
                out.append('<pre class="code"%s><code>%s</code></pre>' % (cls, html.escape(code)))
            i = j + 1
            continue
        m = re.match(r"(#{1,4})\s+(.*)", l)
        if m:
            flush()
            lvl, title = len(m.group(1)), m.group(2).strip()
            hid = slug(title)
            while hid in ids:
                hid += "-1"
            ids.add(hid)
            ctx["section"] = html.unescape(re.sub(r"<[^>]+>", "", inline(title, ctx)))
            if lvl in (2, 3):
                toc.append((lvl, hid, inline(title, ctx)))
            if lvl == 1:
                out.append("<h1>%s</h1>" % inline(title, ctx))
            else:
                out.append('<h%d id="%s">%s<a class="anchor" href="#%s" aria-label="Link to this section">#</a></h%d>' % (lvl, hid, inline(title, ctx), hid, lvl))
            i += 1
            continue
        if s.startswith("|") and i + 1 < len(lines) and re.match(r"\s*\|?\s*:?-{3,}", lines[i + 1]):
            flush()
            j = i
            while j < len(lines) and lines[j].strip().startswith("|"):
                j += 1
            out.append(table(lines[i:j], ctx))
            i = j
            continue
        if s.startswith(">"):
            flush()
            j = i
            q = []
            while j < len(lines) and lines[j].strip().startswith(">"):
                q.append(lines[j].strip()[1:].lstrip())
                j += 1
            out.append('<div class="callout">%s</div>' % md("\n".join(q), ctx)[0])
            i = j
            continue
        if re.match(r"\s*(?:[-*]|\d+\.)\s+", l) and not re.match(r"\s*\*\*", l):
            flush()
            h, i = list_block(i)
            out.append(h)
            continue
        if s in ("---", "***"):
            flush()
            out.append("<hr>")
            i += 1
            continue
        if s.startswith("<") and not s.startswith(("<br", "<kbd", "<code")):
            flush()
            j = i
            while j < len(lines) and lines[j].strip():
                j += 1
            out.append("\n".join(lines[i:j]))
            i = j
            continue
        if not s:
            flush()
            i += 1
            continue
        para.append(s)
        i += 1
    flush()
    return "\n".join(out), toc


# Click a diagram to see it full size: a view over the page that zooms (buttons, + - 0 keys, Ctrl and the
# wheel, pinch on phones) and pans (scroll or drag). No library. Ctrl- or middle-click still opens the SVG
# in a new tab, and without JavaScript the link does that too.
ZOOM_JS = r"""(function(){
var links=document.querySelectorAll('a.zoom');if(!links.length)return;
var box=document.createElement('div');box.className='lightbox';box.hidden=true;
box.setAttribute('role','dialog');box.setAttribute('aria-modal','true');box.setAttribute('aria-label','Diagram, full size');
box.innerHTML='<div class="lb-bar"><button type="button" data-z="out" aria-label="Zoom out">−</button><span class="lb-pct" aria-live="polite"></span><button type="button" data-z="in" aria-label="Zoom in">+</button><button type="button" data-z="fit">Fit</button><button type="button" data-z="one">Actual size</button><a class="lb-tab" target="_blank" rel="noopener">Open in new tab</a><button type="button" class="lb-close" aria-label="Close">×</button></div><div class="lb-view" tabindex="-1"><img alt=""></div>';
document.body.appendChild(box);
var view=box.querySelector('.lb-view'),img=view.querySelector('img'),pct=box.querySelector('.lb-pct'),tab=box.querySelector('.lb-tab');
var w=0,h=0,z=1,back=null;
function fit(){return Math.min((view.clientWidth-60)/w,(view.clientHeight-60)/h);}
function set(nz,cx,cy){nz=Math.max(0.2,Math.min(6,nz));
  var r=view.getBoundingClientRect();cx=cx==null?r.width/2:cx-r.left;cy=cy==null?r.height/2:cy-r.top;
  var ox=img.offsetLeft,oy=img.offsetTop,px=(view.scrollLeft+cx-ox)/z,py=(view.scrollTop+cy-oy)/z;
  z=nz;img.style.width=Math.round(w*z)+'px';img.style.height=Math.round(h*z)+'px';pct.textContent=Math.round(z*100)+'%';
  view.scrollLeft=px*z+img.offsetLeft-cx;view.scrollTop=py*z+img.offsetTop-cy;}
function open(a){w=+a.dataset.w;h=+a.dataset.h;img.src=a.getAttribute('href');img.alt=a.querySelector('img').alt;tab.href=a.getAttribute('href');
  back=a;box.hidden=false;document.documentElement.classList.add('lb-on');
  var f=fit();set(f>=1?Math.min(f,2):1);view.scrollLeft=0;view.scrollTop=0;box.querySelector('.lb-close').focus();}
function close(){box.hidden=true;document.documentElement.classList.remove('lb-on');img.removeAttribute('src');if(back)back.focus();}
for(var i=0;i<links.length;i++)links[i].addEventListener('click',function(e){if(e.ctrlKey||e.metaKey||e.shiftKey||e.button)return;e.preventDefault();open(this);});
box.addEventListener('click',function(e){var b=e.target.closest('button');
  if(b){var k=b.dataset.z;if(k==='in')set(z*1.25);else if(k==='out')set(z/1.25);else if(k==='fit')set(fit());else if(k==='one')set(1);else close();return;}
  if(e.target===view&&!moved)close();});
document.addEventListener('keydown',function(e){if(box.hidden)return;
  if(e.key==='Escape')close();else if(e.key==='+'||e.key==='=')set(z*1.25);else if(e.key==='-')set(z/1.25);else if(e.key==='0')set(fit());else return;e.preventDefault();});
view.addEventListener('wheel',function(e){if(!e.ctrlKey&&!e.metaKey)return;e.preventDefault();set(z*(e.deltaY<0?1.15:1/1.15),e.clientX,e.clientY);},{passive:false});
var drag=null,moved=false;
view.addEventListener('pointerdown',function(e){moved=false;if(e.pointerType!=='mouse'||e.button)return;drag={x:e.clientX,y:e.clientY,l:view.scrollLeft,t:view.scrollTop};});
window.addEventListener('pointermove',function(e){if(!drag)return;var dx=e.clientX-drag.x,dy=e.clientY-drag.y;if(Math.abs(dx)+Math.abs(dy)>3){moved=true;view.classList.add('lb-drag');}
  view.scrollLeft=drag.l-dx;view.scrollTop=drag.t-dy;});
window.addEventListener('pointerup',function(){drag=null;view.classList.remove('lb-drag');});
})();"""

# ---------------------------------------------------------------- Page chrome

LOGO = '<svg class="mark" aria-hidden="true" width="%d" height="%d" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="13" fill="#1d5b4f"/><g fill="none" stroke="#fbfbfa" stroke-width="7"><path d="M15 10v42"/><circle cx="25.5" cy="40" r="8.5"/></g><rect x="40" y="45" width="12" height="7" fill="#7cc4b1"/></svg>'

GH_ICON = '<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16"><path fill="currentColor" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>'


def head(title, desc, extra=""):
    return """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>%s</title>
<meta name="description" content="%s">
<link rel="alternate" type="text/plain" href="https://brevi-lang.org/llms.txt" title="llms.txt">
<link rel="icon" href="/favicon.ico" sizes="32x32">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="manifest" href="/site.webmanifest">
<meta name="theme-color" content="#1d5b4f">
<link rel="preload" href="/plex-sans.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/style.css?v=__CSSV__">%s
</head>""" % (html.escape(title), html.escape(desc, quote=True), extra)


def header(active, site="org", public=False):
    base = "" if site == "org" else "https://brevi-lang.org"
    links = [
        ("docs", base + "/docs/", "Docs"),
        ("agents", base + "/docs/agents", "For agents"),
        ("benchmarks", base + "/docs/benchmarks", "Benchmarks"),
        ("run", "https://brevi.run" if site == "org" else "/", "brevi.run"),
    ]
    # On a phone the least needed link is hidden (it is in the footer too), so the bar fits without scrolling.
    opt = "run" if site == "org" else "benchmarks"
    nav = "".join('<a href="%s"%s%s>%s</a>' % (u, ' class="opt"' if k == opt else "", ' aria-current="page"' if k == active else "", t) for k, u, t in links)
    if public:
        nav += '<a class="gh" href="%s" rel="noopener">%sGitHub</a>' % (GITHUB, GH_ICON)
    home = "/" if site == "org" else "https://brevi-lang.org/"
    name = "Brevi" if site == "org" else "brevi.run"
    tag = '<span class="ver">v__VERSION__</span>' if site == "org" else '<span class="ver">hosted, coming</span>'
    return """<a class="skip" href="#main">Skip to content</a>
<header class="top">
  <div class="wrap bar">
    <a class="brand" href="%s">%s%s%s</a>
    <nav class="main-nav" aria-label="Main">%s</nav>
  </div>
</header>""" % ("/" if site == "run" else home, LOGO % (26, 26), name, tag, nav)


def footer(site="org", public=False):
    gh = '<a href="%s" rel="noopener">GitHub</a>' % GITHUB if public else ""
    base = "" if site == "org" else "https://brevi-lang.org"
    return """<footer class="site">
  <div class="wrap cols">
    <div class="about">
      <span class="brand small">%sBrevi</span>
      <p>The fast, safe target for AI-written Python. Apache-2.0.</p>
    </div>
    <div><h4>Docs</h4><a href="%s/docs/install">Install</a><a href="%s/docs/usage">Using Brevi</a><a href="%s/docs/agents">For agents</a><a href="%s/docs/architecture">Architecture</a></div>
    <div><h4>Project</h4><a href="%s/docs/benchmarks">Benchmarks</a><a href="%s/docs/roadmap">Roadmap</a><a href="%s/docs/changelog">Changelog</a><a href="%s/docs/faq">FAQ</a><a href="%s/pricing">Pricing</a><a href="%s/docs/contributing">Contributing</a></div>
    <div><h4>Elsewhere</h4>%s<a href="https://brevi.run">brevi.run</a><a href="%s/llms.txt">llms.txt</a></div>
  </div>
</footer>""" % (LOGO % (20, 20), base, base, base, base, base, base, base, base, base, base, gh, base)


def status_line(public):
    if public:
        return 'Public preview, v__VERSION__. Open source under Apache-2.0: <a href="%s" rel="noopener">source and releases on GitHub</a>.' % GITHUB
    return "Preview, v__VERSION__. The compiler, written in Rust, opens under the Apache-2.0 license with the public release."


# ---------------------------------------------------------------- Build

def fill(t, version, guide_tokens, public, cssv):
    t = blocks_for(t, public)
    return (t.replace("__STATUS__", status_line(public)).replace("__FULLVERSION__", FULLVERSION or version).replace("__VERSION__", version)
             .replace("__GUIDE_TOKENS__", guide_tokens).replace("__CSSV__", cssv)
             .replace("__GITHUB__", GITHUB))


def page_source(src, public):
    if src.startswith("repo:"):
        text = open(os.path.join(R, src[5:])).read()
    else:
        text = open(os.path.join(S, "docs", src)).read()
    return blocks_for(text, public).replace("__GITHUB__", GITHUB)


def build(out, out_run, version, guide_tokens, public):
    css = open(os.path.join(S, "style.css")).read()
    cssv = hashlib.sha1(css.encode()).hexdigest()[:8]
    os.makedirs(os.path.join(out, "docs", "img"), exist_ok=True)
    open(os.path.join(out, "style.css"), "w").write(css)
    open(os.path.join(out_run, "style.css"), "w").write(css)
    F = lambda t: fill(t, version, guide_tokens, public, cssv)

    # Home and brevi.run
    for src, dst, site, active in [(os.path.join(S, "index.html"), os.path.join(out, "index.html"), "org", "home"),
                                    (os.path.join(S, "pricing.html"), os.path.join(out, "pricing.html"), "org", "pricing"),
                                    (os.path.join(S, "run", "index.html"), os.path.join(out_run, "index.html"), "run", "run")]:
        t = open(src).read().replace("__HEADER__", header(active, site, public)).replace("__FOOTER__", footer(site, public))
        open(dst, "w").write(F(t))

    for dst, site in [(out, "org"), (out_run, "run")]:
        nf = """%s
<body>
%s
<main id="main" class="wrap" style="padding-block: 80px 96px;">
  <p class="eyebrow">404</p>
  <h1 style="font: 500 2.2rem/1.2 var(--serif); margin: 0 0 12px;">There is no page here.</h1>
  <p class="lede">It may have moved when the site was reorganised. Try the <a href="https://brevi-lang.org/docs/">docs</a> or the <a href="/">home page</a>.</p>
</main>
%s
</body>
</html>
""" % (head("Page not found · Brevi", "Page not found."), header("", site, public), footer(site, public))
        open(os.path.join(dst, "404.html"), "w").write(F(nf))

    pages = [(g, slug_, title, src) for g, items in NAV for slug_, title, src in items]
    repo_pages = {s_: src for _, s_, _, src in pages if src.startswith("repo:")}
    for n, (group, slug_, title, src) in enumerate(pages):
        text = page_source(src, public)
        ctx = {"public": public, "repo_pages": repo_pages,
               "repo_dir": os.path.dirname(src[5:]) if src.startswith("repo:") else None}
        # Repo files keep their own first heading out; the page title is the nav title.
        body_md = re.sub(r"\A\s*# .*\n", "", text)
        h1m = re.match(r"\s*# (.*)", text)
        intro = INTROS.get(src, "")
        desc = ""
        if not src.startswith("repo:"):
            dm = re.search(r"<!--\s*description:\s*(.*?)\s*-->", text)
            desc = dm.group(1) if dm else ""
            body_md = re.sub(r"<!--\s*description:.*?-->\n?", "", body_md)
        body, toc = md(body_md, ctx, line_per_para=src in LINE_PER_PARA)
        h1 = inline(h1m.group(1), ctx) if h1m and not src.startswith("repo:") else html.escape(title)
        lead = '<p class="lead">%s</p>' % inline(intro, ctx) if intro else ""
        side = []
        for g, items in NAV:
            side.append('<p class="group">%s</p><ul>' % html.escape(g))
            for s_, t_, _ in items:
                href = "/docs/" if s_ == "index" else "/docs/" + s_
                cur = ' aria-current="page"' if s_ == slug_ else ""
                side.append('<li><a href="%s"%s>%s</a></li>' % (href, cur, html.escape(t_)))
            side.append("</ul>")
        toc_html = ""
        if len([t for t in toc if t[0] == 2]) >= 2:
            toc_html = '<nav class="toc" aria-label="On this page"><p class="group">On this page</p><ul>%s</ul></nav>' % "".join(
                '<li class="l%d"><a href="#%s">%s</a></li>' % (lvl, hid, re.sub(r"<a [^>]*>|</a>", "", t)) for lvl, hid, t in toc if lvl == 2)
        prev_ = pages[n - 1] if n > 0 else None
        next_ = pages[n + 1] if n + 1 < len(pages) else None
        link = lambda p: "/docs/" if p[1] == "index" else "/docs/" + p[1]
        pn = '<nav class="prevnext" aria-label="Previous and next">%s%s</nav>' % (
            '<a class="prev" href="%s"><span>Previous</span>%s</a>' % (link(prev_), html.escape(prev_[2])) if prev_ else "<span></span>",
            '<a class="next" href="%s"><span>Next</span>%s</a>' % (link(next_), html.escape(next_[2])) if next_ else "<span></span>")
        src_link = ""
        if src.startswith("repo:") and public:
            src_link = ' · <a href="%s/blob/main/%s" rel="noopener">source</a>' % (GITHUB, src[5:])
        meta = '<p class="page-meta"><a href="/docs/%s.md">View as Markdown</a>%s</p>' % (slug_, src_link)
        current = 'Docs menu: <b>%s</b>' % html.escape(title)
        doc = """%s
<body class="docs">
%s
<div class="wrap docs-grid">
  <aside class="side" aria-label="Docs">
    <details class="side-menu" id="side-menu">
      <summary>%s</summary>
      <nav>%s</nav>
    </details>
  </aside>
  <main id="main" class="prose">
    <p class="crumb">%s</p>
    <h1>%s</h1>
    %s
    %s
    %s
    %s
  </main>
  %s
</div>
%s
<script>
// Open the docs menu on wide screens (it is a sidebar there); keep it folded on phones.
(function(){var d=document.getElementById('side-menu');var m=window.matchMedia('(min-width: 900px)');function s(){if(m.matches)d.open=true;}s();m.addEventListener&&m.addEventListener('change',s);})();
%s
</script>
</body>
</html>
""" % (head("%s · Brevi docs" % title, desc or "Brevi documentation: %s." % title),
       header("agents" if slug_ == "agents" else "benchmarks" if slug_ == "benchmarks" else "docs", "org", public),
       current, "".join(side), html.escape(group), h1, lead, meta, body, pn, toc_html, footer("org", public),
       ZOOM_JS if 'class="zoom"' in body else "")
        name = "index" if slug_ == "index" else slug_
        open(os.path.join(out, "docs", name + ".html"), "w").write(F(doc))
        # The Markdown twin, for agents: the source as written, with repo: links made absolute or plain.
        md_text = text
        if intro and src.startswith("repo:"):
            md_text = re.sub(r"\A(\s*# .*\n)", lambda m: m.group(1) + "\n" + intro + "\n", md_text)
        def md_link(m):
            u = fix_url(m.group(2), ctx)
            if u is None:
                return m.group(1)
            if u.startswith("/"):
                u = "https://brevi-lang.org" + u
            return "[%s](%s)" % (m.group(1), u)
        md_text = re.sub(r"(?<!!)\[([^\]]+)\]\(([^)\s]+)\)", md_link, md_text)
        open(os.path.join(out, "docs", slug_ + ".md"), "w").write(F(md_text))

    # Diagrams used by the architecture page.
    img = os.path.join(S, "docs", "img")
    if os.path.isdir(img):
        for f in os.listdir(img):
            if f.endswith(".svg"):
                open(os.path.join(out, "docs", "img", f), "w").write(svg_natural(open(os.path.join(img, f)).read()))
    for w in warnings:
        print("warning:", w, file=sys.stderr)
    return [p[1] for p in pages]


if __name__ == "__main__":
    out, out_run, version, guide_tokens, public = sys.argv[1:6]
    FULLVERSION = sys.argv[6] if len(sys.argv) > 6 else ""
    slugs = build(out, out_run, version, guide_tokens, public == "1")
    print("docs pages:", " ".join(slugs))
