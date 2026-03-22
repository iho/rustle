//! yocto_attach detector — port of detectors/yocto_attach.cpp
//!
//! Finds privileged functions (those that call predecessor_account_id and
//! compare with PartialEq) that lack an `assert_one_yocto` check.
//! Skips known callback functions (read from .callback.tmp).
//!
//! Output: $TMP_DIR/.yocto-attach.tmp  (format: funcname@filename)

use near_core::{
    ir::{all_instructions, is_func_privileged, is_inst_call_func_rec, Context, Module},
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: yocto_attach <bitcode_file> [...]");
        std::process::exit(1);
    }

    // Read callback function names from .callback.tmp
    let tmp_dir = std::env::var("TMP_DIR").unwrap_or_else(|_| "./.tmp".to_string());
    let callback_file = format!("{}/.callback.tmp", tmp_dir.trim_end_matches('/'));
    let callbacks: HashSet<String> = fs::read_to_string(&callback_file)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.split('@').next().unwrap_or(l).to_string())
        .collect();

    let ctx = Context::new();
    let writer = TmpWriter::new("yocto-attach");
    let re_assert_yocto = patterns::assert_one_yocto();

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
            // Skip callback functions
            if callbacks.contains(func.name()) {
                continue;
            }

            if !is_func_privileged(func) {
                continue;
            }

            // Check if function (or any callee) calls assert_one_yocto
            let mut func_filename = String::new();
            let mut found_yocto_check = false;

            for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                    _ => continue,
                };
                if func_filename.is_empty() {
                    func_filename = loc.filename.clone();
                }
                if is_inst_call_func_rec(inst, re_assert_yocto) {
                    found_yocto_check = true;
                    break;
                }
            }

            if !found_yocto_check {
                eprintln!(
                    "\x1b[33m[!] yocto_attach: no assert_one_yocto in {}\x1b[0m",
                    func.name()
                );
                writer.write_func_file(func.name(), &func_filename);
            }
        }
    }
}
