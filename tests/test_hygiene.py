"""Pins for TS-10, standard library only and synthetic fixtures only."""

import ast
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "thirdshift"


def imported_modules():
    names = set()
    for path in PACKAGE.glob("*.py"):
        tree = ast.parse(path.read_text())
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    names.add(alias.name.split(".")[0])
            elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
                names.add(node.module.split(".")[0])
    return names


def test_package_imports_only_the_standard_library_and_itself():
    allowed = set(sys.stdlib_module_names) | {"thirdshift"}
    offenders = imported_modules() - allowed
    assert offenders == set(), f"non standard library imports found, {sorted(offenders)}"


EMAIL_PATTERN = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
HOME_PATH_PATTERN = re.compile(r"(/home/[A-Za-z]|[A-Za-z]:\\Users\\)")
HOSTNAME_PATTERN = re.compile(r"(https?://|www\.|[A-Za-z0-9-]+\.(com|net|org|io|dev|ai)\b)")
MARKERS = (
    ("email address", EMAIL_PATTERN),
    ("absolute home path", HOME_PATH_PATTERN),
    ("real looking hostname", HOSTNAME_PATTERN),
)


def test_examples_and_fixtures_carry_no_real_world_markers():
    targets = [path for path in (ROOT / "examples").rglob("*") if path.is_file()]
    targets += list((ROOT / "tests").glob("*.py"))
    offenders = []
    for path in targets:
        text = path.read_text()
        if path.name == Path(__file__).name:
            # The marker patterns are defined in this file, so its own
            # lines would match themselves. Scan the rest of the file.
            text = "\n".join(
                line for line in text.splitlines() if "PATTERN = " not in line
            )
        for marker_name, pattern in MARKERS:
            if pattern.search(text):
                offenders.append((str(path.relative_to(ROOT)), marker_name))
    assert offenders == [], f"non synthetic markers found, {offenders}"
