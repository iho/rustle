#!/usr/bin/env python3
"""
Generate a SARIF 2.1.0 report from Rustle's .tmp detector output files.

Usage (via Makefile / rustle script):
    python3 utils/sarif.py <src_dir>

Environment variables:
    TMP_DIR    — directory containing .<detector>.tmp files
    CSV_PATH   — output directory (sarif.json written here)
"""
import glob
import json
import os
import sys

TMP_PATH = os.environ.get("TMP_DIR", ".tmp")
CSV_PATH = os.environ.get("CSV_PATH", "./audit-result")
PROJ_PATH = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("NEAR_SRC_DIR", ".")

RUSTLE_VERSION = "1.0.0"
RUSTLE_INFO_URI = "https://github.com/blocksecteam/rustle"

# ---------------------------------------------------------------------------
# Detector metadata: id → {level, short_desc, full_desc}
# level maps to SARIF: "error" | "warning" | "note"
# ---------------------------------------------------------------------------
DETECTOR_META = {
    # High severity → error
    "reentrancy": {
        "level": "error",
        "short": "Reentrancy",
        "desc": "State changes after an outgoing promise can be re-entered by the callee's callback.",
    },
    "unsafe-math": {
        "level": "error",
        "short": "Unsafe arithmetic",
        "desc": "Arithmetic operation may overflow or underflow without a checked variant.",
    },
    "unhandled-promise": {
        "level": "error",
        "short": "Unhandled promise",
        "desc": "A Promise is created but dropped without being returned or chained; the call result is ignored.",
    },
    "non-private-callback": {
        "level": "error",
        "short": "Non-private callback",
        "desc": "A callback function is not marked #[private], allowing any account to invoke it directly.",
    },
    "self-transfer": {
        "level": "error",
        "short": "Self-transfer not checked",
        "desc": "Token transfer does not reject transfers where sender == receiver.",
    },
    "incorrect-json-type": {
        "level": "error",
        "short": "Incorrect JSON type",
        "desc": "A u64/u128 value is exposed in the public ABI without string-wrapping, risking precision loss.",
    },
    "unsaved-changes": {
        "level": "error",
        "short": "Unsaved state changes",
        "desc": "Modified data structure is never written back to contract storage.",
    },
    "nft-approval-check": {
        "level": "error",
        "short": "Missing NFT approval check",
        "desc": "nft_transfer / nft_transfer_call does not verify the approval_id.",
    },
    "nft-owner-check": {
        "level": "error",
        "short": "Missing NFT owner check",
        "desc": "nft_approve / nft_revoke does not verify the caller is the token owner.",
    },
    "state-change-before-call": {
        "level": "error",
        "short": "State change before cross-contract call",
        "desc": "Function mutates contract state then fires a cross-contract call without a callback; "
                "failed calls do not roll back state on NEAR.",
    },
    "unchecked-promise-result": {
        "level": "error",
        "short": "Unchecked promise result",
        "desc": "Callback reads promise results without calling is_promise_success(), "
                "treating failed upstream calls as successful.",
    },
    "missing-owner-check": {
        "level": "error",
        "short": "Missing owner check",
        "desc": "Public function sends NEAR tokens or deploys code without verifying the caller is the contract owner.",
    },
    "promise-chain": {
        "level": "error",
        "short": "Hardcoded promise result index",
        "desc": "Callback accesses env::promise_result() with a hardcoded literal index, "
                "risking incorrect behavior when multiple promises complete in unexpected order.",
    },
    # Medium severity → warning
    "div-before-mul": {
        "level": "warning",
        "short": "Division before multiplication",
        "desc": "Integer division before multiplication causes precision loss due to truncation.",
    },
    "round": {
        "level": "warning",
        "short": "Rounding in financial math",
        "desc": "Float rounding operation used in financial calculation.",
    },
    "lock-callback": {
        "level": "warning",
        "short": "Lock not released in callback",
        "desc": "Mutex/lock acquired before a cross-contract call is not released in the callback.",
    },
    "yocto-attach": {
        "level": "warning",
        "short": "Missing assert_one_yocto",
        "desc": "Privileged function does not require the one-yocto deposit to prevent unauthorized calls.",
    },
    "dup-collection-id": {
        "level": "warning",
        "short": "Duplicate collection prefix",
        "desc": "Two NEAR SDK collections share the same storage prefix, causing data corruption.",
    },
    "unregistered-receiver": {
        "level": "warning",
        "short": "Unregistered receiver",
        "desc": "FT transfer does not check whether the receiver is registered for storage.",
    },
    # Low severity → note
    "prepaid-gas": {
        "level": "note",
        "short": "No prepaid-gas check",
        "desc": "Callback does not verify prepaid gas, which may cause it to run out of gas.",
    },
    "non-callback-private": {
        "level": "note",
        "short": "Non-callback marked #[private]",
        "desc": "Function marked #[private] is not a callback; the restriction may be unintentional.",
    },
    "upgrade-func": {
        "level": "note",
        "short": "Upgrade function detected",
        "desc": "Contract contains an upgrade/migration function.",
    },
    "storage-gas": {
        "level": "note",
        "short": "Storage expansion without gas check",
        "desc": "Collection insert/extend does not check available gas for storage expansion.",
    },
    "unclaimed-storage-fee": {
        "level": "note",
        "short": "Unclaimed storage deposit",
        "desc": "Storage registration does not refund excess deposit to the caller.",
    },
    # Info severity → note
    "inconsistency": {
        "level": "note",
        "short": "Naming inconsistency",
        "desc": "Function name does not match the naming convention implied by its implementation.",
    },
    "timestamp": {
        "level": "note",
        "short": "Timestamp dependency",
        "desc": "Business logic depends on env::block_timestamp() which validators can manipulate slightly.",
    },
    "ext-call": {
        "level": "note",
        "short": "External call detected",
        "desc": "Cross-contract function call detected.",
    },
    "promise-result": {
        "level": "note",
        "short": "Promise result accessed",
        "desc": "Function reads a cross-contract promise result.",
    },
    "transfer": {
        "level": "note",
        "short": "Token transfer detected",
        "desc": "Cross-contract token transfer detected.",
    },
    "complex-loop": {
        "level": "note",
        "short": "Complex loop",
        "desc": "Loop contains many instructions and may exhaust gas on large inputs.",
    },
    "public-interface": {
        "level": "note",
        "short": "Public function without near_bindgen",
        "desc": "pub fn exists outside an #[near_bindgen] impl block.",
    },
    "tautology": {
        "level": "note",
        "short": "Tautological condition",
        "desc": "Conditional expression is always true or always false.",
    },
    "unused-ret": {
        "level": "note",
        "short": "Unused return value",
        "desc": "Return value of a function call is discarded.",
    },
}

# ---------------------------------------------------------------------------
# Tmp-file parsers → list of {rule_id, file, line, func}
# ---------------------------------------------------------------------------

def _parse_func_file_line(detector: str, findings: list):
    """func@file@line format"""
    path = os.path.join(TMP_PATH, f".{detector}.tmp")
    try:
        with open(path) as f:
            for raw in f:
                parts = raw.strip().split("@")
                if len(parts) < 3:
                    continue
                func, file, line = parts[0], parts[1], parts[2]
                findings.append({"rule": detector, "func": func, "file": file, "line": int(line)})
    except FileNotFoundError:
        pass


def _parse_func_file(detector: str, findings: list):
    """func@file format (no line number)"""
    path = os.path.join(TMP_PATH, f".{detector}.tmp")
    try:
        with open(path) as f:
            for raw in f:
                parts = raw.strip().split("@")
                if len(parts) < 2:
                    continue
                func, file = parts[0], parts[1]
                findings.append({"rule": detector, "func": func, "file": file, "line": None})
    except FileNotFoundError:
        pass


def _parse_func_bool(detector: str, findings: list):
    """func@bool format — emit finding when bool is False"""
    path = os.path.join(TMP_PATH, f".{detector}.tmp")
    try:
        with open(path) as f:
            for raw in f:
                parts = raw.strip().split("@")
                if len(parts) < 2:
                    continue
                func, ok = parts[0], parts[1]
                if ok.lower() == "false":
                    findings.append({"rule": detector, "func": func, "file": None, "line": None})
    except FileNotFoundError:
        pass


def _parse_func_file_note(detector: str, findings: list):
    """func@file@note format (incorrect-json-type)"""
    path = os.path.join(TMP_PATH, f".{detector}.tmp")
    try:
        with open(path) as f:
            for raw in f:
                parts = raw.strip().split("@")
                if len(parts) < 3:
                    continue
                func, file = parts[0], parts[1]
                findings.append({"rule": detector, "func": func, "file": file, "line": None})
    except FileNotFoundError:
        pass


def _collect_findings() -> list:
    findings = []

    # func@file@line
    for det in [
        "reentrancy", "ext-call", "promise-result", "complex-loop",
        "transfer", "round", "div-before-mul", "unsafe-math", "timestamp",
        "unhandled-promise", "unsaved-changes", "state-change-before-call",
        "unchecked-promise-result", "missing-owner-check", "promise-chain",
    ]:
        _parse_func_file_line(det, findings)

    # func@file
    for det in ["upgrade-func", "yocto-attach", "lock-callback",
                "non-callback-private", "non-private-callback", "public-interface"]:
        _parse_func_file(det, findings)

    # func@bool (False = problem)
    for det in [
        "self-transfer", "prepaid-gas", "storage-gas", "unregistered-receiver",
        "unclaimed-storage-fee", "nft-approval-check", "nft-owner-check",
    ]:
        _parse_func_bool(det, findings)

    # func@file@note
    _parse_func_file_note("incorrect-json-type", findings)

    return findings


# ---------------------------------------------------------------------------
# SARIF builder
# ---------------------------------------------------------------------------

def _make_rule(det_id: str) -> dict:
    meta = DETECTOR_META.get(det_id, {"level": "note", "short": det_id, "desc": det_id})
    sarif_level = meta["level"]  # error/warning/note
    return {
        "id": det_id,
        "name": meta["short"].replace(" ", ""),
        "shortDescription": {"text": meta["short"]},
        "fullDescription": {"text": meta["desc"]},
        "defaultConfiguration": {"level": sarif_level},
        "helpUri": f"{RUSTLE_INFO_URI}#detector-{det_id}",
    }


def _make_result(finding: dict, rules_index: dict) -> dict:
    det_id = finding["rule"]
    meta = DETECTOR_META.get(det_id, {"level": "note", "short": det_id, "desc": det_id})
    result = {
        "ruleId": det_id,
        "ruleIndex": rules_index[det_id],
        "level": meta["level"],
        "message": {"text": meta["desc"]},
    }

    file = finding.get("file")
    line = finding.get("line")
    func = finding.get("func", "")

    if func:
        result["message"]["text"] += f" (function: {func})"

    if file:
        # Make path relative to project root
        if file.startswith("/"):
            try:
                file = os.path.relpath(file, PROJ_PATH)
            except ValueError:
                pass
        location = {
            "physicalLocation": {
                "artifactLocation": {"uri": file, "uriBaseId": "%SRCROOT%"},
            }
        }
        if line is not None:
            location["physicalLocation"]["region"] = {"startLine": line}
        result["locations"] = [location]

    return result


def build_sarif(findings: list) -> dict:
    # Collect unique rule IDs actually used
    used_ids = list(dict.fromkeys(f["rule"] for f in findings))
    all_ids = list(DETECTOR_META.keys())
    # Rules list: used detectors first (preserving order), then unused ones for completeness
    rule_ids_ordered = used_ids + [d for d in all_ids if d not in used_ids]
    rules = [_make_rule(rid) for rid in rule_ids_ordered]
    rules_index = {rid: i for i, rid in enumerate(rule_ids_ordered)}

    results = [_make_result(f, rules_index) for f in findings]

    return {
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "Rustle",
                        "version": RUSTLE_VERSION,
                        "informationUri": RUSTLE_INFO_URI,
                        "rules": rules,
                    }
                },
                "originalUriBaseIds": {
                    "%SRCROOT%": {"uri": f"file://{os.path.abspath(PROJ_PATH)}/"}
                },
                "results": results,
            }
        ],
    }


def main():
    # Demangle function names in tmp files (same as audit.py)
    for tmp_file in glob.glob(os.path.join(TMP_PATH, ".*.tmp")):
        if not os.path.exists(tmp_file + ".org"):
            os.system(f"mv {tmp_file} {tmp_file}.org; rustfilt -i {tmp_file}.org -o {tmp_file}; rm {tmp_file}.org 2>/dev/null || mv {tmp_file}.org {tmp_file}")

    findings = _collect_findings()
    sarif = build_sarif(findings)

    os.makedirs(CSV_PATH, exist_ok=True)
    out_path = os.path.join(CSV_PATH, "results.sarif")
    with open(out_path, "w") as f:
        json.dump(sarif, f, indent=2)

    n = len(findings)
    print(f"[rustle] SARIF report written to {out_path} ({n} finding{'s' if n != 1 else ''})")


if __name__ == "__main__":
    main()
