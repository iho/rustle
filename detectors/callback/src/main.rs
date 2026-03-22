//! callback detector — port of detectors/callback.cpp
//!
//! Identifies callback functions by checking whether they call any
//! `promise_result` / `promise_results_count` / `is_promise_success` API.
//!
//! Output: $TMP_DIR/.callback.tmp  (format: funcname@filename)

use near_core::{
    ir::{all_instructions, is_inst_call_func, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: callback <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("callback");

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

            let mut func_filename = String::new();

            'inst_loop: for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                    _ => continue,
                };

                if func_filename.is_empty() {
                    func_filename = loc.filename.clone();
                }

                if is_inst_call_func(inst, patterns::promise_result()) {
                    eprintln!(
                        "\x1b[33m[*] callback: {} @ {}\x1b[0m",
                        func.name(),
                        func_filename
                    );
                    writer.write_func_file(func.name(), &func_filename);
                    break 'inst_loop;
                }
            }
        }
    }
}
