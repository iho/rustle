# Rustle C++ → Rust Migration Plan

## Current Architecture

27 C++ LLVM pass detectors compiled as `.so` plugins, loaded via:
```
opt -enable-new-pm=0 -load detectors/callback.so -callback file.bc -o /dev/null
```

**By pass type:**
- 11 `FunctionPass` (per-function analysis)
- 11 `ModulePass` (module-level + call graph)
- 1 `LoopPass` (`complex_loop.cpp`)
- 1 shared utility (`near_core.cpp/h`, ~500 lines)

---

## Core Problem with Direct Translation

The current plugin model requires registering with LLVM's **legacy pass manager** (`-enable-new-pm=0`). In LLVM 19 this is deprecated, and neither `inkwell` nor `llvm-sys` expose the `RegisterPass<T>` / `PassManagerBuilder` C++ APIs needed to produce compatible `.so` plugins. Bridging this from Rust would require a thin C shim anyway, defeating the purpose.

---

## Chosen Approach: Standalone Rust Binaries

Rewrite each detector as a **standalone Rust binary** that loads and analyzes bitcode directly. Removes the legacy PM constraint entirely.

**Current invocation:**
```bash
opt -enable-new-pm=0 -load detectors/callback.so -callback file.bc -o /dev/null
```

**New invocation:**
```bash
./detectors/callback file.bc
```

The Makefile and `rustle` script get updated to call binaries instead of `opt --load`. Everything downstream (`.tmp` files, `audit.py`) stays identical.

---

## Phase 1 — Foundation (Weeks 1–3)

**`Cargo.toml` (workspace root)**
- Add `llvm-sys = "190"` (matches LLVM 19)
- Set up a `detectors/` Rust workspace with one crate per detector

**Port `near_core` to a shared Rust crate (`detectors/near_core/`)**
- `regex_patterns.rs` — all precompiled `llvm::Regex` patterns → Rust `regex::Regex`
- `ir_utils.rs` — `simpleFindUsers`, `findUsers`, `isFuncPrivileged`, `isInstCallFunc`, `isFuncCallFuncRec`, `findFunctionCallerRec` using `llvm-sys` raw IR traversal
- `logger.rs` — colored output matching current Logger class
- `output.rs` — `.tmp` file append helper

**Validation milestone:** minimal test binary opens `.bc` via `llvm-sys`, iterates functions/instructions, outputs to `.tmp`, full pipeline still passes unit tests.

---

## Phase 2 — Simple Detectors (Weeks 4–6)

Pattern-matching-only detectors. APIs used: `Function`, `BasicBlock`, `Instruction`, `CallBase`, `DebugLoc`, `Regex` only.

- `round`, `promise_result`, `timestamp`, `self_transfer`

Each: `main.rs` opens `argv[1]` as bitcode → iterates module functions → filters by `regexForLibFunc`/`regexForLibLoc` → appends matches to `.tmp`.

Establishes the repeatable Rust detector pattern.

---

## Phase 3 — FunctionPass Detectors (Weeks 7–12)

| Detector | Key added complexity |
|---|---|
| `div_before_mul`, `prepaid_gas`, `yocto_attach` | Simple value tracking |
| `unhandled_promise` | `Value::users()` traversal |
| `reentrancy` | `SwitchInst` + `StoreInst` path analysis |
| `unsafe_math` | `BinaryOperator` opcode inspection |
| `callback`, `all_call` | Cross-function output used by other detectors |
| `nft_approval_check` | Recursive parameter tracking (most complex FunctionPass) |

---

## Phase 4 — ModulePass Detectors (Weeks 13–18)

Call graph via `llvm-sys` C API (`LLVMCreateCallGraph`).

Ordering dependency preserved:
- `ext_call_trait` must run before `ext_call`
- `callback` must run before `nft_owner_check`, `yocto_attach`

Detectors: `ext_call_trait`, `ext_call`, `nft_owner_check`, `storage_gas`, `upgrade_func`, `unsaved_changes`, `unclaimed_storage_fee`, `transfer`, `nep_interface`, `unregistered_receiver`, `timestamp` (ModulePass variant)

---

## Phase 5 — LoopPass & Integration (Weeks 19–22)

`complex_loop` is the only LoopPass. Without the legacy PM, loop detection uses dominator/loop header analysis via `llvm-sys` loop analysis APIs. Highest-risk detector.

Fallback: keep `complex_loop.cpp` in C++ temporarily if loop analysis is too costly to port.

Full pipeline integration, unit test suite validation, performance profiling vs. C++ baseline.

---

## What Changes

- `detectors/Makefile` — build Rust binaries instead of `.so` files
- `Makefile` — replace `opt --load detectors/X.so -X` with `./detectors/X`
- `rustle` (bash script) — update invocation per detector

## What Stays Unchanged

- `.tmp` file format — identical output
- `utils/audit.py` — no changes
- `rustle` script structure — only the `opt --load` lines change
- `examples/` and `scripts/unit_test.sh` — identical

---

## Key Decisions

1. **`llvm-sys` over `inkwell`** — `llvm-sys` gives full C API coverage needed for raw IR traversal; `inkwell` has ergonomics gaps for this use case.
2. **One binary per detector** — preserves current parallel execution model.
3. **`complex_loop` fallback** — may remain C++ if LoopPass analysis is too costly to port in Phase 5.

---

## Additional Features (from plan.md, lower priority)

### A. SARIF / GitHub Code Scanning Output
- New `utils/sarif.py` reads `.tmp` files, outputs `results.sarif` (SARIF 2.1.0)
- Extract shared parsing into `utils/findings.py` (shared by `audit.py` and `sarif.py`)
- Add `audit-report-sarif` Makefile target
- Add `--format sarif` flag to `rustle` script
- Severity mapping: `high` → `error`, `medium` → `warning`, `low`/`info` → `note`

### B. `.rustle.toml` Suppression Config
- New `utils/suppression.py`: `find_rustle_toml()`, `load_suppressions()`, `is_suppressed(detector, file, func)`
- Modify `utils/audit.py` to load suppressions and filter findings
- Config format:
  ```toml
  [suppress]
  detectors = ["timestamp"]
  [[suppress.rule]]
  detector = "unsafe-math"
  path = "src/math_helpers.rs"
  ```

### C. Promise Chaining Detector (new C++ → eventually Rust)
- Detect `Promise::and(p1, p2)` without `.then(callback)`
- Detect `promise_result(0)` with hardcoded index after `Promise::and`
- New `detectors/promise_chain.cpp` (or `.rs` once Phase 2 is done)
- Add to `high` severity group in `rustle` script

---

## Detector Inventory

**FunctionPass (migrate in Phase 2–3):**
`all_call`, `callback`, `div_before_mul`, `nft_approval_check`, `prepaid_gas`, `promise_result`, `reentrancy`, `round`, `self_transfer`, `struct_member`, `unhandled_promise`, `unregistered_receiver`, `unsafe_math`, `yocto_attach`

**ModulePass (migrate in Phase 4):**
`ext_call`, `ext_call_trait`, `nep_interface`, `nft_owner_check`, `storage_gas`, `timestamp`, `transfer`, `unclaimed_storage_fee`, `unsaved_changes`, `upgrade_func`

**LoopPass (migrate in Phase 5, fallback to C++):**
`complex_loop`

**Shared utility (migrate first in Phase 1):**
`near_core`
