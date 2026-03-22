//! self_transfer detector — port of detectors/self_transfer.cpp
//!
//! For each ft_transfer / ft_transfer_call trait implementation, checks whether
//! sender_id != receiver_id is verified (preventing self-transfers).
//!
//! Output: $TMP_DIR/.self-transfer.tmp  (format: funcname@True / funcname@False)

use llvm_sys::core::{LLVMGetArgOperand, LLVMGetCalledValue, LLVMGetNumArgOperands, LLVMIsAFunction, LLVMIsACallInst, LLVMIsAInvokeInst};
use near_core::{
    ir::{all_instructions, is_inst_call_func, simple_find_users, Context,
         FunctionRef, Module, raw_value_name},
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;

/// Check whether function `func` (or a callee it delegates `receiver_arg` to)
/// performs a sender/receiver equality check. Mirrors `hasSenderReceiverCheck`.
fn has_sender_receiver_check(func: FunctionRef, receiver_offset: u32) -> bool {
    // Standard library implementations always check — treat as safe
    if patterns::ft_transfer_call_standard().is_match(func.name()) {
        return true;
    }

    if func.param_count() <= receiver_offset {
        return false;
    }

    // Collect all users of the receiver argument
    let receiver_param = func.get_param(receiver_offset);
    let mut receiver_users: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
    simple_find_users(receiver_param, &mut receiver_users, false, false);

    for inst in all_instructions(func) {
        if is_inst_call_func(inst, patterns::partial_eq()) {
            // Check if any argument has an AccountId or String type
            let n = inst.num_args();
            for i in 0..n {
                let ty = inst.arg_type_string(i);
                if ty.contains("near_sdk::types::account_id::AccountId")
                    || ty.contains("alloc::string::String")
                {
                    return true;
                }
            }
        } else if inst.is_call() {
            // Check if receiver_id is passed into a callee — recurse
            let raw = inst.raw();
            unsafe {
                let n = LLVMGetNumArgOperands(raw);
                let callee = LLVMGetCalledValue(raw);
                if callee.is_null() || LLVMIsAFunction(callee).is_null() {
                    continue;
                }
                if LLVMIsACallInst(raw).is_null() && LLVMIsAInvokeInst(raw).is_null() {
                    continue;
                }
                let mut next_receiver_offset: Option<u32> = None;
                for i in 0..n {
                    let arg = LLVMGetArgOperand(raw, i);
                    if receiver_users.contains(&arg) {
                        next_receiver_offset = Some(i);
                        break;
                    }
                }
                if let Some(off) = next_receiver_offset {
                    let callee_name = raw_value_name(callee);
                    // Find the FunctionRef by matching the name in the current module
                    // We use a workaround: build a FunctionRef from the callee value directly
                    let callee_func = FunctionRef(callee);
                    if !callee_name.is_empty()
                        && has_sender_receiver_check(callee_func, off)
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: self_transfer <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("self-transfer");
    let re_transfer = patterns::ft_transfer_trait();
    let re_transfer_call = patterns::ft_transfer_call_trait();

    for path in &args[1..] {
        let module = match Module::from_bitcode(&ctx, path) {
            Ok(m) => m,
            Err(e) => { eprintln!("warning: {e}"); continue; }
        };

        for func in module.functions() {
            if re_transfer.is_match(func.name()) {
                if func.param_count() < 3 {
                    continue;
                }
                eprintln!("\x1b[33m[*] Find ft_transfer {}\x1b[0m", func.name());
                // receiver_id is arg index 1 (self=0, receiver_id=1, amount=2)
                let ok = has_sender_receiver_check(func, 1);
                if ok {
                    eprintln!("\x1b[33m[*] self-transfer check present\x1b[0m");
                } else {
                    eprintln!("\x1b[33m[!] self-transfer check missing\x1b[0m");
                }
                writer.write_bool(func.name(), ok);
            } else if re_transfer_call.is_match(func.name()) {
                if func.param_count() < 3 {
                    continue;
                }
                eprintln!("\x1b[33m[*] Find ft_transfer_call {}\x1b[0m", func.name());
                // receiver_id is arg index 2 (self=0, receiver_id=1... wait C++ uses 2 for call)
                let ok = has_sender_receiver_check(func, 2);
                if ok {
                    eprintln!("\x1b[33m[*] self-transfer check present for ft_transfer_call\x1b[0m");
                } else {
                    eprintln!("\x1b[33m[!] self-transfer check missing for ft_transfer_call\x1b[0m");
                }
                writer.write_bool(func.name(), ok);
            }
        }
    }
}
