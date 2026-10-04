#!/usr/bin/env python3
"""Writes THIRD-PARTY-NOTICES.txt: the licences of the works built into the plug-in's
binaries, which their licences ask to accompany every copy (decisions.md R17).

The crates are those `ca72-plugin` links (normal dependencies, not build scripts, tests or
procedural macros) on any platform it is released for, read from `cargo metadata` and
Cargo.lock, with the licence files each crate ships; a crate that ships none gets its
licence's standard text with its authors. The Rust standard library, built into every Rust
binary, and the panel's font follow. Run it after any change to the dependencies or to
rust-toolchain.toml:

    python3 scripts/notices.py           # writes THIRD-PARTY-NOTICES.txt
    python3 scripts/notices.py --check   # fails if the file is out of date (CI)
"""

import hashlib
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "THIRD-PARTY-NOTICES.txt")
PLATFORMS = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-pc-windows-gnu",
    "x86_64-unknown-linux-gnu",
]
FONT = "third_party/urw-gothic"
FONT_COPYRIGHT = "Copyright 2014 by (URW)++ Design & Development"
LICENCE_FILE = re.compile(r"^(licen[cs]e|copying|notice|unlicense|copyright)", re.I)

MIT = """Copyright (c) {authors}

Permission is hereby granted, free of charge, to any person obtaining a copy of this
software and associated documentation files (the "Software"), to deal in the Software
without restriction, including without limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
"""


def metadata(platform):
    out = subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", platform],
        cwd=ROOT,
    )
    return json.loads(out)


def linked(meta):
    """The packages `ca72-plugin` links, its own workspace's left out."""
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    workspace = set(meta["workspace_members"])
    root = next(i for i in workspace if packages[i]["name"] == "ca72-plugin")
    seen, stack = set(), [root]
    while stack:
        i = stack.pop()
        if i in seen:
            continue
        seen.add(i)
        for dep in nodes[i]["deps"]:
            if not any(k["kind"] is None for k in dep["dep_kinds"]):
                continue
            p = packages[dep["pkg"]]
            if any("proc-macro" in t["kind"] for t in p["targets"]):
                continue
            stack.append(dep["pkg"])
    return [packages[i] for i in seen if i not in workspace]


# The Rust project's LICENSE-MIT (rust-lang/rust): std, core, alloc and the crates the
# standard library bundles are linked into every binary, under MIT OR Apache-2.0.
RUST_MIT = """Copyright (c) The Rust Project Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy of this
software and associated documentation files (the "Software"), to deal in the Software
without restriction, including without limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
"""


def toolchain():
    """The Rust release rust-toolchain.toml names."""
    with open(os.path.join(ROOT, "rust-toolchain.toml"), encoding="utf-8") as f:
        return re.search(r'^channel\s*=\s*"([^"]+)"', f.read(), re.M).group(1)


def where(p):
    """Where the crate's source is: its repository, or for one Cargo.lock takes from git, the
    repository and the commit."""
    src = p.get("source") or ""
    if src.startswith("git+"):
        url, _, rev = src[4:].partition("#")
        return f"{url.split('?')[0]} at {rev[:12]}"
    return p.get("repository") or ""


def text(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        lines = f.read().replace("\r\n", "\n").split("\n")
    return "\n".join(l.rstrip() for l in lines).strip("\n") + "\n"


def mit_elected(expr):
    """Whether the crate offers MIT as one choice among others (`A OR B`, or the old `A/B`):
    the plug-in takes it under MIT, and only that licence's notice is due."""
    terms = [t.strip("() ") for t in re.split(r"\s+OR\s+|/", expr)]
    return "MIT" in terms and not re.search(r"\bAND\b", expr)


def licences(p):
    """The crate's licence texts: its files, else its licence's standard text."""
    d = os.path.dirname(p["manifest_path"])
    names = sorted(f for f in os.listdir(d) if LICENCE_FILE.match(f) and os.path.isfile(os.path.join(d, f)))
    if p.get("license_file"):
        names.append(os.path.relpath(os.path.join(d, p["license_file"]), d))
    names = list(dict.fromkeys(names))
    expr = p.get("license") or ""
    mit = mit_elected(expr)
    if mit:
        names = [n for n in names if "mit" in n.lower()] or [
            n for n in names if "Permission is hereby granted" in text(os.path.join(d, n))
        ]
    texts = [t for t in (text(os.path.join(d, n)) for n in names) if t.strip()]
    if texts:
        return texts
    if mit or expr == "MIT":
        authors = ", ".join(re.sub(r"\s*<[^>]*>", "", a) for a in p.get("authors") or []) or f"the {p['name']} authors"
        return [MIT.format(authors=authors)]
    sys.exit(f"notices.py: {p['name']} {p['version']} ({expr}) ships no licence file and is not MIT")


def render():
    crates = {}
    for platform in PLATFORMS:
        for p in linked(metadata(platform)):
            crates[(p["name"], p["version"])] = p
    groups = {}
    for key in sorted(crates):
        p = crates[key]
        body = "\n\n".join(licences(p))
        digest = hashlib.sha256(body.encode()).hexdigest()
        groups.setdefault(digest, (body, []))[1].append(p)

    out = []
    out.append("THIRD-PARTY NOTICES FOR THE CA-72\n")
    out.append(
        "The CA-72, copyright © 2026 Idle Foundry Ltd., is free software under the GNU General\n"
        "Public License, version 3 or later (LICENSE); its source is at\n"
        "https://github.com/idlefoundry/ca-72. Its plug-ins are built with the works below, each\n"
        "under its own licence, whose notices follow. The crates are those Cargo.lock names, from\n"
        "crates.io; the sources of the ones it takes from git repositories are in each release as\n"
        "well, CA-72-<version>-git-sources.tar.gz.\n"
        "Written by scripts/notices.py from Cargo.lock: do not edit by hand.\n"
    )
    out.append("CRATES\n")
    for key in sorted(crates):
        p = crates[key]
        expr = p.get("license") or "see below"
        out.append(f"  {p['name']} {p['version']}  ({expr}" + (", used under MIT)" if mit_elected(expr) and expr != "MIT" else ")"))
    out.append("")
    out.append(f"  The Rust standard library, Rust {toolchain()}  (MIT OR Apache-2.0, used under MIT)")
    out.append(f"  URW Gothic  (AGPL-3.0 with a font exception)\n")
    for body, members in sorted(groups.values(), key=lambda g: (g[1][0]["name"], g[1][0]["version"])):
        out.append("=" * 78)
        for p in members:
            src = where(p)
            out.append(f"{p['name']} {p['version']}" + (f"  {src}" if src else ""))
        out.append("-" * 78)
        out.append(body)
    out.append("=" * 78)
    out.append(f"The Rust standard library (std, core, alloc and the crates it bundles), Rust {toolchain()}")
    out.append("  https://github.com/rust-lang/rust")
    out.append("-" * 78)
    out.append(RUST_MIT)
    out.append("=" * 78)
    out.append("URW Gothic, the panel's lettering, by (URW)++  https://github.com/ArtifexSoftware/urw-base35-fonts")
    out.append(FONT_COPYRIGHT)
    out.append("-" * 78)
    out.append(
        "Built into the plug-ins under the GNU Affero General Public License, version 3, which\n"
        "section 13 of the GNU GPL, version 3, lets a work combine with it. The font exception\n"
        "below concerns documents that embed the font.\n"
    )
    font = os.path.join(ROOT, FONT)
    out.append(text(os.path.join(font, "LICENSE")))
    out.append(text(os.path.join(font, "COPYING")))
    return "\n".join(out)


def main():
    notices = render()
    if "--check" in sys.argv[1:]:
        try:
            current = open(OUT, encoding="utf-8").read()
        except FileNotFoundError:
            current = ""
        if current != notices:
            sys.exit("THIRD-PARTY-NOTICES.txt is out of date: run python3 scripts/notices.py")
        return
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(notices)


if __name__ == "__main__":
    main()
