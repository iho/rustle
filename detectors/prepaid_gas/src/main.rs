//! prepaid_gas detector — port of detectors/prepaid_gas.cpp
//!
//! For each ft_transfer_call implementation, checks whether prepaid_gas() is
//! called and its result is compared via PartialOrd (i.e., a gas-limit guard).
//!
//! Output: $TMP_DIR/.prepaid-gas.tmp  (format: funcname@True / funcname@False)

use near_core::{
    ir::{all_instructions, is_inst_call_func, simple_find_users, Context, InstructionRef, Module},
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: prepaid_gas <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("prepaid-gas");
    let re_ft_call = patterns::ft_transfer_call_trait();
    let re_prepaid = patterns::prepaid_gas();
    let re_ord = patterns::partial_ord();

    for path in &args[1..] {
        let module = match Module::from_bitcode(&ctx, path) {
            Ok(m) => m,
            Err(e) => { eprintln!("warning: {e}"); continue; }
        };

        for func in module.functions() {
            if !re_ft_call.is_match(func.name()) {
                continue;
            }
            eprintln!("\x1b[33m[*] Find ft_transfer_call {}\x1b[0m", func.name());

            let mut has_check = false;

            'outer: for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) => l,
                    None => continue,
                };
                if patterns::is_lib_loc(&loc.filename) {
                    continue;
                }
                if !is_inst_call_func(inst, re_prepaid) {
                    continue;
                }

                // Found a prepaid_gas() call — trace all its users
                let mut gas_users: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
                simple_find_users(inst.raw(), &mut gas_users, false, false);

                for &u in &gas_users {
                    let u_inst = InstructionRef(u);
                    if is_inst_call_func(u_inst, re_ord) {
                        has_check = true;
                        break 'outer;
                    }
                }
            }

            if has_check {
                eprintln!("\x1b[33m[*] Found prepaid_gas check\x1b[0m");
            } else {
                eprintln!("\x1b[33m[!] Lacking prepaid_gas check\x1b[0m");
            }
            writer.write_bool(func.name(), has_check);
        }
    }
}
