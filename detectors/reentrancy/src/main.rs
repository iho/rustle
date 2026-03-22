//! reentrancy detector — port of detectors/reentrancy.cpp
//!
//! Detects state changes (StoreInst to `self` fields) inside the
//! PromiseResult::Successful branch of a callback function, which can
//! lead to reentrancy vulnerabilities.
//!
//! Output: $TMP_DIR/.reentrancy.tmp  (format: funcname@filename@line)

use llvm_sys::core::{
    LLVMConstIntGetZExtValue, LLVMGetBasicBlockName, LLVMGetNumOperands, LLVMGetOperand,
    LLVMIsAReturnInst,
};
use llvm_sys::prelude::LLVMBasicBlockRef;
use near_core::{
    ir::{
        all_instructions, is_inst_call_func, raw_bb_instructions, simple_find_users, Context,
        InstructionRef, Module,
    },
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;

/// Walk the basic-block chain starting at `start_bb` (the PromiseResult::Successful
/// successor), following unconditional "bb*"-named successors, and report the
/// first StoreInst that writes to a `self` field and is not used in a return.
fn check_reentrant_store(
    func: near_core::ir::FunctionRef,
    start_bb: LLVMBasicBlockRef,
    writer: &TmpWriter,
) -> bool {
    let self_param = func.get_param(0);
    let mut self_users: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
    simple_find_users(self_param, &mut self_users, false, false);

    let mut current_bb = start_bb;

    loop {
        let mut next_bb: Option<LLVMBasicBlockRef> = None;

        for inst in raw_bb_instructions(current_bb) {
            // Follow branch successors whose basic block names start with "bb"
            if inst.is_branch() {
                for i in 0..inst.num_successors() {
                    let succ = inst.get_successor(i);
                    if succ.is_null() {
                        continue;
                    }
                    let name = unsafe {
                        let ptr = LLVMGetBasicBlockName(succ);
                        if ptr.is_null() {
                            ""
                        } else {
                            std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or("")
                        }
                    };
                    if name.starts_with("bb") {
                        next_bb = Some(succ);
                    }
                }
            }

            // Check StoreInst: pointer operand 1
            if inst.is_store() {
                let store_ptr = inst.get_operand(1);
                if store_ptr.is_null() {
                    continue;
                }

                // Is the store location used in a return?
                let mut store_ptr_users: HashSet<llvm_sys::prelude::LLVMValueRef> =
                    HashSet::new();
                simple_find_users(store_ptr, &mut store_ptr_users, false, false);
                let used_in_return = store_ptr_users
                    .iter()
                    .any(|&u| unsafe { !LLVMIsAReturnInst(u).is_null() });

                // Is the store location a user of `self`?
                let use_self = self_users.contains(&store_ptr);

                if !used_in_return && use_self {
                    if let Some(loc) = inst.debug_loc() {
                        if !patterns::is_lib_loc(&loc.filename) {
                            eprintln!(
                                "\x1b[33m[!] reentrancy: state change after PromiseResult::Successful in {} @ {}:{}\x1b[0m",
                                func.name(),
                                loc.filename,
                                loc.line
                            );
                            writer.write(func.name(), &loc.filename, loc.line);
                            return true;
                        }
                    }
                }
            }
        }

        match next_bb {
            Some(bb) => current_bb = bb,
            None => break,
        }
    }
    false
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: reentrancy <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("reentrancy");

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
            if func.param_count() == 0 {
                continue;
            }

            for inst in all_instructions(func) {
                match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => {}
                    _ => continue,
                }

                if !is_inst_call_func(inst, patterns::promise_result()) {
                    continue;
                }

                // Found a promise_result call.
                // arg[0] is the sret pointer (where PromiseResult enum is stored).
                if inst.num_args() == 0 {
                    continue;
                }
                let arg0 = inst.get_arg(0);

                // Find all transitive users of arg0 (disable cross-function tracking).
                let mut pm_rs_users: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
                simple_find_users(arg0, &mut pm_rs_users, false, true);

                // Find a SwitchInst among users with case value 1 (PromiseResult::Successful).
                let mut successful_bb: Option<LLVMBasicBlockRef> = None;
                'switch_search: for &user in &pm_rs_users {
                    let user_inst = InstructionRef(user);
                    if !user_inst.is_switch() {
                        continue;
                    }
                    // Switch operands: 0=condition, 1=default, (2+2i)=case_val, (2+2i+1)=case_dest
                    let n_ops = unsafe { LLVMGetNumOperands(user) as usize };
                    let n_cases = n_ops.saturating_sub(2) / 2;
                    for i in 0..n_cases {
                        let case_val_op = unsafe { LLVMGetOperand(user, (2 + 2 * i) as u32) };
                        if case_val_op.is_null() {
                            continue;
                        }
                        let val = unsafe { LLVMConstIntGetZExtValue(case_val_op) };
                        if val == 1 {
                            // successor index i+1 corresponds to case i (0 = default)
                            let succ_bb = user_inst.get_successor((i + 1) as u32);
                            if !succ_bb.is_null() {
                                successful_bb = Some(succ_bb);
                                break 'switch_search;
                            }
                        }
                    }
                }

                if let Some(bb) = successful_bb {
                    check_reentrant_store(func, bb, &writer);
                    // Per the C++ original: return after processing the promise_result call,
                    // regardless of whether reentrancy was found.
                    continue 'func_loop;
                }

                // SDK 5 fallback: `match PromiseResult` compiles to icmp+br instead of switch.
                // Find conditional branch instructions among the transitive users of the sret
                // alloca and check all their successors for reentrant stores.
                let mut found_branch = false;
                for &user in &pm_rs_users {
                    let user_inst = InstructionRef(user);
                    if !user_inst.is_branch() {
                        continue;
                    }
                    let n_succ = user_inst.num_successors();
                    if n_succ < 2 {
                        continue; // unconditional branch — not what we want
                    }
                    for i in 0..n_succ {
                        let succ_bb = user_inst.get_successor(i);
                        if succ_bb.is_null() {
                            continue;
                        }
                        check_reentrant_store(func, succ_bb, &writer);
                    }
                    found_branch = true;
                    break;
                }
                if found_branch {
                    continue 'func_loop;
                }
            }
        }
    }
}
