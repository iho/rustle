//! transfer detector — port of detectors/transfer.cpp
//!
//! Finds all functions that perform a NEAR token transfer or NEP-141 token
//! transfer (directly or via a callee).
//!
//! Output: $TMP_DIR/.transfer.tmp  (format: funcname@filename@line)

use near_core::{
    ir::{all_instructions, is_inst_call_func_rec, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: transfer <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("transfer");
    let re_transfer = patterns::promise_transfer();
    let re_nep141 = patterns::nep141_transfer();

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

                if is_inst_call_func_rec(inst, re_transfer) {
                    eprintln!(
                        "\x1b[33m[!] promise transfer in {} @ {}:{}\x1b[0m",
                        func.name(),
                        loc.filename,
                        loc.line
                    );
                    writer.write(func.name(), &loc.filename, loc.line);
                } else if is_inst_call_func_rec(inst, re_nep141) {
                    eprintln!(
                        "\x1b[33m[!] NEP-141 transfer in {} @ {}:{}\x1b[0m",
                        func.name(),
                        loc.filename,
                        loc.line
                    );
                    writer.write(func.name(), &loc.filename, loc.line);
                }
            }
        }
    }
}
