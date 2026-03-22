#!/usr/bin/env python3
"""
Suppression config loader for Rustle.

Reads .rustle.toml from the project root and provides is_suppressed()
so that audit.py can skip matching findings.

Config format (.rustle.toml):
    # Suppress an entire detector for all files/functions
    [suppress]
    detectors = ["timestamp", "complex-loop"]

    # Suppress a specific detector for a file (all functions in that file)
    [[suppress.rule]]
    detector = "unsafe-math"
    path = "src/math_helpers.rs"

    # Suppress a specific detector for a named function
    [[suppress.rule]]
    detector = "div-before-mul"
    function = "calculate_fee"

    # Suppress a specific detector for a function in a specific file
    [[suppress.rule]]
    detector = "reentrancy"
    path = "src/lib.rs"
    function = "withdraw"
"""

import os
import re

try:
    import tomllib  # Python 3.11+
except ImportError:
    try:
        import tomli as tomllib  # pip install tomli for older Python
    except ImportError:
        tomllib = None  # type: ignore


def _parse_toml_minimal(text: str) -> dict:
    """Minimal TOML parser for the subset used in .rustle.toml.
    Falls back when neither tomllib nor tomli is available."""
    result = {"suppress": {"detectors": [], "rule": []}}
    suppress = result["suppress"]

    # Extract detectors = [...]
    det_match = re.search(r'detectors\s*=\s*\[([^\]]*)\]', text, re.DOTALL)
    if det_match:
        raw = det_match.group(1)
        suppress["detectors"] = [
            s.strip().strip('"').strip("'")
            for s in raw.split(",")
            if s.strip().strip('"').strip("'")
        ]

    # Extract [[suppress.rule]] blocks
    blocks = re.split(r'\[\[suppress\.rule\]\]', text)[1:]
    for block in blocks:
        rule = {}
        for key in ("detector", "path", "function"):
            m = re.search(rf'{key}\s*=\s*["\']([^"\']+)["\']', block)
            if m:
                rule[key] = m.group(1)
        if "detector" in rule:
            suppress["rule"].append(rule)

    return result


class Suppressions:
    """Loaded set of suppression rules."""

    def __init__(self, config: dict):
        sup = config.get("suppress", {})
        # Global detector suppressions
        self._global: set = set(sup.get("detectors", []))
        # Per-rule suppressions: list of {detector, path?, function?}
        self._rules: list = sup.get("rule", [])

    def is_suppressed(self, detector: str, file: str = "", func: str = "") -> bool:
        """Return True if this finding should be suppressed."""
        if detector in self._global:
            return True
        for rule in self._rules:
            if rule.get("detector") != detector:
                continue
            rule_path = rule.get("path", "")
            rule_func = rule.get("function", "")
            path_match = (not rule_path) or file.endswith(rule_path) or rule_path in file
            func_match = (not rule_func) or func == rule_func or func.endswith(rule_func)
            if path_match and func_match:
                return True
        return False


def load_suppressions(project_root: str) -> Suppressions:
    """Find and load .rustle.toml from project_root. Returns empty Suppressions if not found."""
    toml_path = os.path.join(project_root, ".rustle.toml")
    if not os.path.exists(toml_path):
        return Suppressions({})

    with open(toml_path, "rb" if tomllib else "r") as f:
        content = f.read()

    try:
        if tomllib:
            if isinstance(content, bytes):
                config = tomllib.loads(content.decode())
            else:
                config = tomllib.loads(content)
        else:
            config = _parse_toml_minimal(content if isinstance(content, str) else content.decode())
    except Exception as e:
        print(f"[rustle] Warning: could not parse .rustle.toml: {e}")
        return Suppressions({})

    print(f"[rustle] Loaded suppression config from {toml_path}")
    return Suppressions(config)
