//! unchecked_promise_result detector
//!
//! Detects callback functions that call `env::promise_result()` or
//! `env::promise_results_count()` but never verify the outcome with
//! `is_promise_success()` or `promise_result_as_success()`.
//!
//! Such callbacks blindly process cross-contract results as if they always
//! succeeded.  When the upstream promise fails, the callback still runs
//! but may apply state changes (e.g., credit tokens) that should only happen
//! on success.
//!
//! Output: $TMP_DIR/.unchecked-promise-result.tmp  (format: funcname@filename@line)

use near_core::{
    ir::{all_instructions, is_inst_call_func, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: unchecked_promise_result <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("unchecked-promise-result");
    let re_access = patterns::promise_result_access();
    let re_check = patterns::promise_result_check();

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

            let mut access_loc: Option<(String, u32)> = None;
            let mut has_check = false;

            for inst in all_instructions(func) {
                if !inst.is_call() {
                    continue;
                }
                if is_inst_call_func(inst, re_check) {
                    has_check = true;
                    break;
                }
                if access_loc.is_none() && is_inst_call_func(inst, re_access) {
                    if let Some(loc) = inst.debug_loc() {
                        if !patterns::is_lib_loc(&loc.filename) {
                            access_loc = Some((loc.filename, loc.line));
                        }
                    }
                }
            }

            if has_check {
                continue;
            }

            let (filename, line) = match access_loc {
                Some(loc) => loc,
                None => continue,
            };

            eprintln!(
                "\x1b[33m[!] unchecked-promise-result in {} @ {}:{}\x1b[0m",
                func.name(),
                filename,
                line
            );
            writer.write(func.name(), &filename, line);
        }
    }
}
