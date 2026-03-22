//! div_before_mul detector — port of detectors/div_before_mul.cpp
//!
//! Finds integer/float division whose result flows into a multiplication,
//! causing precision loss. Uses `simple_find_users` to trace the div result.
//!
//! Output: $TMP_DIR/.div-before-mul.tmp

use llvm_sys::core::LLVMIsAInstruction;
use near_core::{
    ir::{all_instructions, simple_find_users, Context, InstructionRef, Module},
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: div_before_mul <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("div-before-mul");

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

                if !inst.is_div() {
                    continue;
                }

                // Find all transitive users of the division result
                let mut users: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
                simple_find_users(inst.raw(), &mut users, true, false);

                let found = users.iter().any(|&u| {
                    // Only consider instructions with a non-library debug loc
                    let u_inst = InstructionRef(u);
                    unsafe {
                        if LLVMIsAInstruction(u).is_null() {
                            return false;
                        }
                    }
                    if let Some(uloc) = u_inst.debug_loc() {
                        if patterns::is_lib_loc(&uloc.filename) {
                            return false;
                        }
                    }
                    u_inst.is_mul() || u_inst.is_llvm_mul_overflow()
                });

                if found {
                    eprintln!(
                        "\x1b[33m[!] div-before-mul at {}:{}\x1b[0m",
                        loc.filename, loc.line
                    );
                    writer.write(func.name(), &loc.filename, loc.line);
                }
            }
        }
    }
}
