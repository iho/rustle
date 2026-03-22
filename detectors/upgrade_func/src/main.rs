//! upgrade_func detector — port of detectors/upgrade_func.cpp
//!
//! Detects functions that perform an in-place contract upgrade by calling
//! both `promise_batch_action_deploy_contract` and
//! `promise_batch_action_function_call`.
//!
//! Output: $TMP_DIR/.upgrade-func.tmp  (format: funcname@filename)

use near_core::{
    ir::{all_instructions, is_inst_call_func, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: upgrade_func <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("upgrade-func");
    let re_deploy = patterns::promise_batch_deploy();
    let re_func_call = patterns::promise_batch_function_call();

    let mut found_upgrade = false;

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

            let mut call_deploy = false;
            let mut call_func_call = false;
            let mut func_filename = String::new();

            for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                    _ => continue,
                };

                if is_inst_call_func(inst, re_deploy) {
                    if func_filename.is_empty() {
                        func_filename = loc.filename.clone();
                    }
                    call_deploy = true;
                }
                if is_inst_call_func(inst, re_func_call) {
                    call_func_call = true;
                }
            }

            if call_deploy && call_func_call {
                eprintln!(
                    "\x1b[33m[*] upgrade_func: {} @ {}\x1b[0m",
                    func.name(),
                    func_filename
                );
                writer.write_func_file(func.name(), &func_filename);
                found_upgrade = true;
            }
        }
    }

    if !found_upgrade {
        eprintln!("\x1b[33m[*] upgrade_func: no upgrade function found\x1b[0m");
    }
}
