//! unhandled_promise detector — port of detectors/unhandled_promise.cpp
//!
//! Finds Promise objects that are neither returned from the function nor
//! chained with `.then()` / `.and()` / etc.
//!
//! Detection strategy (LLVM 21 / opaque-pointer IR):
//!   With opaque pointers, `Promise` allocas appear as `[16 x i8]` rather
//!   than `%"near_sdk::promise::Promise"`, so string-matching arg0 no longer
//!   works.  Instead we look for `drop_in_place<near_sdk::promise::Promise>`
//!   calls: these are emitted by the Rust compiler whenever a Promise value
//!   goes out of scope without being moved out (i.e., it was neither returned
//!   nor consumed by a chaining method).  We then find the sret call that
//!   produced the dropped Promise and report it.
//!
//! Output: $TMP_DIR/.unhandled-promise.tmp  (format: funcname@filename@line)

use near_core::{
    ir::{all_instructions, is_inst_call_func, value_users, Context, InstructionRef, Module},
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: unhandled_promise <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("unhandled-promise");
    let re_drop = patterns::promise_drop_in_place();

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

            // Collect all drop_in_place<Promise> calls in this function.
            // For each, find the sret call that produced the dropped value.
            // Use a set to avoid duplicate reports for the same alloca.
            let mut reported: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();

            for inst in all_instructions(func) {
                if !inst.is_call() {
                    continue;
                }
                // Is this a drop_in_place<near_sdk::promise::Promise>?
                if !is_inst_call_func(inst, re_drop) {
                    continue;
                }
                if inst.num_args() == 0 {
                    continue;
                }
                let drop_arg = inst.get_arg(0); // ptr to the Promise being dropped

                if !reported.insert(drop_arg) {
                    continue; // already reported for this alloca
                }

                // Find the sret call that wrote into drop_arg.
                // Among all users of drop_arg, find a call that is NOT the
                // drop_in_place itself, and has a valid non-lib debug location.
                for user in value_users(drop_arg) {
                    if user == inst.raw() {
                        continue; // skip the drop_in_place itself
                    }
                    let user_inst = InstructionRef(user);
                    if !user_inst.is_call() {
                        continue;
                    }
                    let loc = match user_inst.debug_loc() {
                        Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                        _ => continue,
                    };
                    eprintln!(
                        "\x1b[33m[!] unhandled promise in {} @ {}:{}\x1b[0m",
                        func.name(),
                        loc.filename,
                        loc.line
                    );
                    writer.write(func.name(), &loc.filename, loc.line);
                    break; // one report per drop
                }
            }
        }
    }
}
