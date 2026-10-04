"""Symbolizes the samples sprof.so wrote: where the time went, by physical function, by
innermost (inlined) function and by source line.

    python3 sym.py <binary> <samples file>

See README.md next to this file.
"""

import collections
import subprocess
import sys

binp, pcs = sys.argv[1], sys.argv[2]
cnt = collections.Counter(line.strip() for line in open(pcs) if line.strip())
addrs = list(cnt)
out = subprocess.run(
    ["llvm-symbolizer", "--obj=" + binp, "--inlining", "--demangle", "--functions=short"],
    input="\n".join("0x" + a for a in addrs) + "\n",
    capture_output=True,
    text=True,
).stdout
blocks = out.strip("\n").split("\n\n")
total = sum(cnt.values())
leaf = collections.Counter()
outer = collections.Counter()
line = collections.Counter()
where = collections.Counter()
for a, b in zip(addrs, blocks):
    ls = b.split("\n")
    # Each frame is a function name and its location; innermost (inlined) first.
    frames = [(ls[i], ls[i + 1]) for i in range(0, len(ls) - 1, 2)]
    c = cnt[a]
    leaf[frames[0][0]] += c
    outer[frames[-1][0]] += c
    where[frames[-1][0] + " (" + frames[-1][1].rsplit("/", 1)[-1].split(":")[0] + ")"] += c
    loc = frames[0][1].rsplit("/", 1)[-1]
    line[loc.rsplit(":", 1)[0]] += c


def show(title, ctr, k=25):
    print("---", title, "---")
    for n, c in ctr.most_common(k):
        print(f"{100 * c / total:5.1f}% {n}")


print("samples", total)
show("physical function", outer)
show("physical function and its file", where, 40)
show("innermost (inlined) function", leaf)
show("source line", line, 40)
