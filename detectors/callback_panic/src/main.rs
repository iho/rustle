//! callback_panic detector
//!
//! Detects callback functions that call plain `.unwrap()` (which panics on None/Err).
//!
//! In NEAR, if a callback panics, its state changes are rolled back but the
//! upstream cross-contract call has already committed.  A callback that uses
//! `.unwrap()` on a storage lookup or external value can therefore leave the
//! contract in an inconsistent state: the external action is permanent, but the
//! accounting update intended to record it was silently reverted.
//!
//! Detection: a function is flagged when it
//!   1. calls `env::promise_result` / `is_promise_success` (i.e. it is a callback), and
//!   2. calls `core::option::unwrap_failed` or `core::result::unwrap_failed`
//!      (the panic path of plain `.unwrap()`; inlined by the compiler in debug builds).
//!
//! Output: $TMP_DIR/.callback-panic.tmp  (format: funcname@filename@line)

use near_core::{
    ir::{all_instructions, is_inst_call_func, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: callback_panic <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("callback-panic");
    let re_promise_result = patterns::promise_result();
    let re_plain_unwrap = patterns::plain_unwrap();

    for path in &args[1..] {
        let module = match Module::from_bitcode(&ctx, path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("warning: {e}");
                continue;
            }
        };

        for func in module.functions() {
            if patterns::is_lib_func(func.name()) {
                continue;
            }

            let mut is_callback = false;
            let mut has_unwrap = false;
            let mut first_loc: Option<(String, u32)> = None;

            for inst in all_instructions(func) {
                if !inst.is_call() {
                    continue;
                }

                // Record the first user-code instruction location (for reporting).
                if first_loc.is_none() {
                    if let Some(loc) = inst.debug_loc() {
                        if !patterns::is_lib_loc(&loc.filename) {
                            first_loc = Some((loc.filename.clone(), loc.line));
                        }
                    }
                }

                // Check if this function is a callback (reads promise result).
                if !is_callback && is_inst_call_func(inst, re_promise_result) {
                    is_callback = true;
                }

                // Check for plain unwrap panic path (inlined as unwrap_failed in debug builds,
                // or as a direct Option::unwrap / Result::unwrap call in optimised builds).
                if !has_unwrap && is_inst_call_func(inst, re_plain_unwrap) {
                    has_unwrap = true;
                }

                if is_callback && has_unwrap && first_loc.is_some() {
                    break;
                }
            }

            if is_callback && has_unwrap {
                if let Some((filename, line)) = first_loc {
                    writer.write(func.name(), &filename, line);
                }
            }
        }
    }
}
