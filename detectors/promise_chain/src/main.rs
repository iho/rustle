use llvm_sys::core::LLVMIsAConstantInt;
use near_core::{
    ir::{all_instructions, is_inst_call_func, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: promise_chain <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("promise-chain");
    let re_access = patterns::promise_result_access();

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

            for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                    _ => continue,
                };

                if !is_inst_call_func(inst, re_access) {
                    continue;
                }

                // promise_result(idx: u64) — idx is the last (or second) argument.
                // The sret pointer is arg 0; the index is arg 1.
                if inst.num_args() < 2 {
                    continue;
                }

                // Check whether the index argument is a constant integer (hardcoded literal).
                let idx_arg = inst.get_arg(1);
                let is_const = unsafe { !LLVMIsAConstantInt(idx_arg).is_null() };

                if is_const {
                    eprintln!(
                        "\x1b[33m[!] promise_chain: hardcoded promise_result index in {} @ {}:{}\x1b[0m",
                        func.name(),
                        loc.filename,
                        loc.line
                    );
                    writer.write(func.name(), &loc.filename, loc.line);
                    break; // one report per function is enough
                }
            }
        }
    }
}
