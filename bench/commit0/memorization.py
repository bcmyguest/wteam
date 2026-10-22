"""How much of a rebuilt library is upstream's code, verbatim?

For every function the Commit0 stub left empty (body is only a docstring and/or `pass`),
compare the rebuilt body with the upstream body:
  sim      difflib ratio of the normalised body lines (docstrings dropped)
  lines    share of upstream's non-trivial body lines that appear verbatim in the rebuild
  comments upstream inline comments reproduced verbatim (the stubs contain none, so a
           reproduced comment can only come from memory)

    python3 memorization.py <stub pkg dir> <upstream pkg dir> <rebuilt pkg dir> [...more rebuilt]
"""
import ast
import difflib
import pathlib
import re
import sys


def functions(path):
    """{qualified name: (body source lines without docstring, node)} for every def in a file."""
    src = path.read_text()
    lines = src.splitlines()
    out = {}

    def visit(node, prefix):
        for child in ast.iter_child_nodes(node):
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                body = child.body
                if body and isinstance(body[0], ast.Expr) and isinstance(getattr(body[0], "value", None), ast.Constant) and isinstance(body[0].value.value, str):
                    body = body[1:]
                text = lines[body[0].lineno - 1: child.end_lineno] if body else []
                out[prefix + child.name] = (text, body)
                visit(child, prefix + child.name + ".")
            elif isinstance(child, ast.ClassDef):
                visit(child, prefix + child.name + ".")
    visit(ast.parse(src), "")
    return out


def is_stub(body):
    return all(isinstance(s, ast.Pass) for s in body)


def norm(lines):
    out = []
    for l in lines:
        code = l.split("#", 1)[0].strip()
        if code and code not in ("pass", "else:", "try:", "return", "}", ")", "]"):
            out.append(re.sub(r"\s+", " ", code))
    return out


def comments(lines):
    return {m.group(1).strip() for l in lines for m in [re.search(r"#\s*(.+)$", l)] if m and len(m.group(1).strip()) > 12}


def compare(stub_dir, up_dir, new_dir):
    sims, line_hits, line_total, com_hits, com_total, n = [], 0, 0, 0, 0, 0
    verbatim_fns = []
    for stub_file in sorted(pathlib.Path(stub_dir).glob("*.py")):
        up_file, new_file = pathlib.Path(up_dir) / stub_file.name, pathlib.Path(new_dir) / stub_file.name
        if not (up_file.exists() and new_file.exists()):
            continue
        try:
            stub, up, new = functions(stub_file), functions(up_file), functions(new_file)
        except SyntaxError:
            continue
        for name, (_, body) in stub.items():
            if not is_stub(body) or name not in up or name not in new:
                continue
            u, r = norm(up[name][0]), norm(new[name][0])
            if len(u) < 3:
                continue
            n += 1
            s = difflib.SequenceMatcher(None, u, r).ratio()
            sims.append(s)
            rset = set(r)
            line_hits += sum(1 for l in u if l in rset)
            line_total += len(u)
            uc, rc = comments(up[name][0]), comments(new[name][0])
            com_hits += len(uc & rc)
            com_total += len(uc)
            if s >= 0.9:
                verbatim_fns.append(f"{stub_file.name}:{name}")
    sims.sort()
    med = sims[len(sims) // 2] if sims else 0
    return {
        "functions": n,
        "median_sim": round(med, 2),
        "fns_sim>=0.9": len(verbatim_fns),
        "verbatim_lines": f"{line_hits}/{line_total} ({line_hits / max(line_total, 1):.0%})",
        "comments_reproduced": f"{com_hits}/{com_total}",
        "examples": verbatim_fns[:6],
    }


if __name__ == "__main__":
    stub, up, *rebuilt = sys.argv[1:]
    for d in rebuilt:
        print(d, compare(stub, up, d))
