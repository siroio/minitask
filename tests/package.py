"""Check a release ZIP after relocation.  Usage: python tests/package.py ZIP [EMACS]"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

repo = Path(__file__).resolve().parents[1]
archive = Path(sys.argv[1]).resolve()
assert archive.is_file(), f"Missing release: {archive}"
assert Path(str(archive) + ".sha256").read_text().split()[0] == hashlib.sha256(archive.read_bytes()).hexdigest()
with tempfile.TemporaryDirectory(prefix="package-", dir=repo / ".cache") as temporary:
    relocated = Path(temporary) / "配布先 with spaces"
    with zipfile.ZipFile(archive) as bundle:
        names = bundle.namelist()
        assert all(not n.startswith("/") and ".." not in n.split("/") and "\\" not in n for n in names)
        bundle.extractall(relocated)
    roots = list(relocated.iterdir())
    assert len(roots) == 1 and roots[0].is_dir()
    root = roots[0]
    for name in ("minitask.exe", "README.md", "LICENSE", "THIRD-PARTY-NOTICES.txt", "locales/ja.json",
                 "emacs/minitask.el", "emacs/locales/ja.json", "skills/using-minitask/SKILL.md", "docs/emacs.md", "yazi-keymap.toml"):
        assert (root / name).is_file(), name
    for file in root.rglob("*"):
        if not file.is_file():
            continue
        relative = file.relative_to(root)
        assert not ({".cache", "target", ".git"} & set(relative.parts)), relative
        assert file.suffix not in (".log", ".elc", ".pdb"), relative
        if relative.parts[0] != "licenses" and file.suffix != ".exe":
            assert repo.as_posix().lower() not in file.read_text(encoding="utf-8").replace("\\", "/").lower(), relative
    environment = {k: v for k, v in os.environ.items() if not k.startswith("MINITASK_")}
    home = Path(temporary) / "保存先"
    (home / "開発").mkdir(parents=True)

    def run(*args):
        result = subprocess.run([str(root / "minitask.exe"), "--home", str(home), *args],
                                cwd=temporary, env=environment, capture_output=True, encoding="utf-8")
        assert result.returncode == 0, result.stderr
        return json.loads(result.stdout)

    assert run("workspaces")["items"][0]["name"] == "開発"
    item = run("add", "--workspace", "開発", "--kind", "note", "--title", "配布検証", "--body", "自由なメモ")["item"]
    assert run("show", "--id", item["id"])["item"]["title"] == "配布検証"
    assert "自由なメモ" in Path(item["path"]).read_text(encoding="utf-8")
    assert run("resume", "--workspace", "開発")["notes"][0]["id"] == item["id"]
    # Version must match the ZIP, and locale assets must resolve beside the EXE.
    catalog_path = root / "locales/ja.json"
    catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
    catalog["main.text_004"] = "package-locale {0}"
    catalog_path.write_text(json.dumps(catalog, ensure_ascii=False), encoding="utf-8")
    version = subprocess.run([str(root / "minitask.exe"), "--version"], cwd=temporary,
                             env=environment, capture_output=True, encoding="utf-8")
    expected_version = root.name.removeprefix("minitask-").removesuffix("-windows-x64")
    assert version.returncode == 0 and version.stdout.strip() == "package-locale " + expected_version, version
    if len(sys.argv) > 2:
        # No PATH discovery: the frontend must find the adjacent packaged binary.
        lisp = root / "verify.el"
        lisp.write_text("(require 'cl-lib)\n"
                        "(cl-letf (((symbol-function 'executable-find) (lambda (&rest _) nil))) (require 'minitask))\n"
                        "(unless (file-equal-p minitask-executable (expand-file-name \"../minitask.exe\" minitask--directory)) (error \"Executable not relocated\"))\n"
                        "(setq minitask-home (getenv \"PACKAGE_TEST_HOME\") minitask-workspace \"開発\")\n"
                        "(minitask)\n(unless (derived-mode-p 'minitask-mode) (error \"Wrong mode\"))\n"
                        "(unless (string-match-p \"配布検証\" (buffer-string)) (error \"Missing Japanese row\"))\n"
                        "(princ \"EMACS_RELOCATED_OK\\n\")\n", encoding="utf-8")
        environment["PACKAGE_TEST_HOME"] = str(home)
        result = subprocess.run([sys.argv[2], "-Q", "--batch", "-L", str(root / "emacs"), "-l", str(lisp)],
                                cwd=temporary, env=environment, capture_output=True)
        assert result.returncode == 0 and b"EMACS_RELOCATED_OK" in result.stdout, result.stderr
print("PASS: checksum, package contents, clean docs, relocated Japanese CLI" + (" and Emacs" if len(sys.argv) > 2 else ""))
