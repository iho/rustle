//! ext_call detector — port of detectors/ext_call.cpp
//!
//! Reads the ext_call_trait list produced by the `ext_call_trait` binary,
//! then finds all functions that call any of those trait functions.
//!
//! Output: $TMP_DIR/.ext-call.tmp  (format: funcname@filename@line)

use near_core::{
    ir::{all_instructions, Context, Module},
    output::TmpWriter,
    patterns,
};
use regex::Regex;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ext_call <bitcode_file> [...]");
        std::process::exit(1);
    }

    // Read the ext-call-trait list
    let tmp_dir = std::env::var("TMP_DIR").unwrap_or_else(|_| "./.tmp".to_string());
    let trait_file = format!("{}/.ext-call-trait.tmp", tmp_dir.trim_end_matches('/'));
    let ext_call_traits: Vec<Regex> = fs::read_to_string(&trait_file)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| Regex::new(l).ok())
        .collect();

    let ctx = Context::new();
    let writer = TmpWriter::new("ext-call");

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

            for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                    _ => continue,
                };

                if let Some(callee_name) = inst.called_fn_name() {
                    for re in &ext_call_traits {
                        if re.is_match(callee_name) {
                            eprintln!(
                                "\x1b[33m[*] ext_call: {} calls {} @ {}:{}\x1b[0m",
                                func.name(),
                                callee_name,
                                loc.filename,
                                loc.line
                            );
                            writer.write(func.name(), &loc.filename, loc.line);
                        }
                    }
                }
            }
        }
    }
}
