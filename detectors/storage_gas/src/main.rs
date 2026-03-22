//! storage_gas detector — port of detectors/storage_gas.cpp
//!
//! Checks whether functions that expand storage (via `insert`/`extend` on
//! NEAR SDK collections) also check `storage_usage()` for gas accounting.
//!
//! Output: $TMP_DIR/.storage-gas.tmp  (format: funcname@True/False)

use near_core::{
    ir::{
        all_instructions, find_function_callers, func_calls_func_rec, is_inst_call_func, Context,
        FunctionRef, Module,
    },
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: storage_gas <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("storage-gas");
    let re_storage_expansion = patterns::storage_expansion();
    let re_storage_usage = patterns::storage_usage();

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

            // Check if function has any storage expansion (insert/extend)
            let has_storage_expansion = all_instructions(func).any(|inst| {
                matches!(inst.debug_loc(), Some(l) if !patterns::is_lib_loc(&l.filename))
                    && is_inst_call_func(inst, re_storage_expansion)
            });

            if !has_storage_expansion {
                continue;
            }

            // Check if function (or its callers) checks storage_usage
            let mut has_gas_check = func_calls_func_rec(func, re_storage_usage);

            if !has_gas_check {
                // Check callers up to depth 2
                let callers = find_function_callers(func, 2);
                for caller_raw in callers {
                    let caller_func = FunctionRef(caller_raw);
                    if func_calls_func_rec(caller_func, re_storage_usage) {
                        has_gas_check = true;
                        break;
                    }
                }
            }

            if has_gas_check {
                eprintln!(
                    "\x1b[33m[*] storage_gas: storage check found in {}\x1b[0m",
                    func.name()
                );
            } else {
                eprintln!(
                    "\x1b[33m[!] storage_gas: no storage check in {}\x1b[0m",
                    func.name()
                );
            }
            writer.write_bool(func.name(), has_gas_check);
        }
    }
}
