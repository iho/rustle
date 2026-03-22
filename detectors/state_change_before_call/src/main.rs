//! state_change_before_call detector
//!
//! Detects functions that mutate contract state (StoreInst to `self` fields)
//! AND make a cross-contract call whose Promise is dropped without a callback.
//!
//! In NEAR, a failed cross-contract call does NOT automatically roll back
//! state changes made before it.  If a function debits a balance, sends
//! tokens via a cross-contract call, and has no callback to restore state
//! on failure, the debit is permanent even when the transfer never arrives.
//!
//! Detection strategy:
//!   1. Find drop_in_place<near_sdk::promise::Promise> calls — these indicate
//!      a cross-contract call whose result is discarded (no callback registered).
//!   2. Check whether the same function also stores to a self field
//!      (StoreInst whose pointer operand is transitively derived from param 0).
//!   3. Report at the location of the cross-contract call.
//!
//! Output: $TMP_DIR/.state-change-before-call.tmp  (format: funcname@filename@line)

use near_core::{
    ir::{
        all_instructions, is_inst_call_func, simple_find_users, value_users, Context,
        InstructionRef, Module,
    },
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: state_change_before_call <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("state-change-before-call");
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
            if func.param_count() == 0 {
                continue;
            }

            // Pass 1: find drop_in_place<Promise> — records the location of the
            // sret call that produced the dropped Promise.
            let mut call_loc: Option<(String, u32)> = None;
            for inst in all_instructions(func) {
                if !inst.is_call() || !is_inst_call_func(inst, re_drop) {
                    continue;
                }
                if inst.num_args() == 0 {
                    continue;
                }
                let drop_arg = inst.get_arg(0);
                for user in value_users(drop_arg) {
                    if user == inst.raw() {
                        continue;
                    }
                    let ui = InstructionRef(user);
                    if !ui.is_call() {
                        continue;
                    }
                    if let Some(loc) = ui.debug_loc() {
                        if !patterns::is_lib_loc(&loc.filename) {
                            call_loc = Some((loc.filename, loc.line));
                            break;
                        }
                    }
                }
                if call_loc.is_some() {
                    break;
                }
            }

            if call_loc.is_none() {
                continue;
            }

            // Pass 2: does this function store to a self field?
            // Collect all values transitively derived from param 0 (self ptr).
            let self_param = func.get_param(0);
            let mut self_users: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
            simple_find_users(self_param, &mut self_users, false, false);

            let has_self_store = all_instructions(func).any(|inst| {
                if !inst.is_store() {
                    return false;
                }
                let ptr = inst.get_operand(1);
                if ptr.is_null() {
                    return false;
                }
                if !self_users.contains(&ptr) {
                    return false;
                }
                matches!(inst.debug_loc(), Some(loc) if !patterns::is_lib_loc(&loc.filename))
            });

            if !has_self_store {
                continue;
            }

            let (filename, line) = call_loc.unwrap();
            eprintln!(
                "\x1b[33m[!] state-change-before-call in {} @ {}:{}\x1b[0m",
                func.name(),
                filename,
                line
            );
            writer.write(func.name(), &filename, line);
        }
    }
}
