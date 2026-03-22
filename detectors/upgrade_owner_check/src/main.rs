//! upgrade_owner_check detector
//!
//! Detects upgrade functions (those that call both
//! `promise_batch_action_deploy_contract` and `promise_batch_action_function_call`)
//! that are NOT protected by an owner check (`predecessor_account_id` comparison).
//!
//! An unguarded upgrade function is a critical vulnerability: any account can
//! deploy arbitrary bytecode to the contract, taking complete control of it.
//!
//! Output: $TMP_DIR/.upgrade-owner-check.tmp  (format: funcname@filename@line)

use near_core::{
    ir::{all_instructions, is_func_privileged, is_inst_call_func, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: upgrade_owner_check <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("upgrade-owner-check");
    let re_deploy = patterns::promise_batch_deploy();
    let re_func_call = patterns::promise_batch_function_call();

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
            if func.name().contains("$closure$") {
                continue;
            }

            let mut call_deploy = false;
            let mut call_func_call = false;
            let mut first_loc: Option<(String, u32)> = None;

            for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                    _ => continue,
                };

                if first_loc.is_none() {
                    first_loc = Some((loc.filename.clone(), loc.line));
                }
                if is_inst_call_func(inst, re_deploy) {
                    call_deploy = true;
                }
                if is_inst_call_func(inst, re_func_call) {
                    call_func_call = true;
                }
            }

            if !(call_deploy && call_func_call) {
                continue;
            }

            let privileged = is_func_privileged(func);
            if privileged {
                eprintln!(
                    "\x1b[33m[*] upgrade-owner-check: owner check present in {}\x1b[0m",
                    func.name()
                );
            } else {
                let (filename, line) = first_loc.unwrap_or_default();
                eprintln!(
                    "\x1b[33m[!] upgrade-owner-check: no owner check in {} @ {}:{}\x1b[0m",
                    func.name(),
                    filename,
                    line
                );
                writer.write(func.name(), &filename, line);
            }
        }
    }
}
