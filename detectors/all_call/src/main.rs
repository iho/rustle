//! all_call detector — port of detectors/all_call.cpp
//!
//! Records every direct function call site (callee, source file, line).
//! Used as input by other detectors and the Python audit layer.
//!
//! Output: $TMP_DIR/.all-call.tmp  (format: callee_name@filename@line)

use near_core::{
    ir::{all_instructions, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: all_call <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("all-call");

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
                    writer.write(callee_name, &loc.filename, loc.line);
                }
            }
        }
    }
}
