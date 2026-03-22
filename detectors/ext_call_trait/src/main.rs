//! ext_call_trait detector — port of detectors/ext_call_trait.cpp
//!
//! Finds all functions that make external NEAR calls (via
//! `Promise::function_call` / `function_call_weight`). The output is
//! consumed by the `ext_call` detector as a list of call-trait patterns.
//!
//! Output: $TMP_DIR/.ext-call-trait.tmp  (format: funcname — one per line)

use near_core::{
    ir::{all_instructions, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ext_call_trait <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("ext-call-trait");
    let re_ext = patterns::ext_call();

    for path in &args[1..] {
        let module = match Module::from_bitcode(&ctx, path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("warning: {e}");
                continue;
            }
        };

        'func_loop: for func in module.functions() {
            if patterns::is_lib_func(func.name()) {
                continue;
            }

            for inst in all_instructions(func) {
                match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => {}
                    _ => continue,
                }

                if let Some(callee_name) = inst.called_fn_name() {
                    if re_ext.is_match(callee_name) {
                        writer.write_name(func.name());
                        continue 'func_loop;
                    }
                }
            }
        }
    }
}
