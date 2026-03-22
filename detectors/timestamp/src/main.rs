//! timestamp detector — port of detectors/timestamp.cpp
//!
//! Warns whenever a function uses env::block_timestamp(), which can be
//! manipulated by validators. Originally a ModulePass; replicated here by
//! iterating all functions in the module.
//!
//! Output: $TMP_DIR/.timestamp.tmp

use near_core::{
    ir::{all_instructions, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: timestamp <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("timestamp");
    let re = patterns::block_timestamp();

    for path in &args[1..] {
        let module = match Module::from_bitcode(&ctx, path) {
            Ok(m) => m,
            Err(e) => { eprintln!("warning: {e}"); continue; }
        };

        for func in module.functions() {
            for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) => l,
                    None => continue,
                };
                if patterns::is_lib_loc(&loc.filename) {
                    continue;
                }
                if let Some(callee) = inst.called_fn_name() {
                    if re.is_match(callee) {
                        eprintln!("\x1b[33m[!] timestamp used at {}:{}\x1b[0m", loc.filename, loc.line);
                        writer.write(func.name(), &loc.filename, loc.line);
                    }
                }
            }
        }
    }
}
