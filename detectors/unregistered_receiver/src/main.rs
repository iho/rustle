//! unregistered_receiver detector — port of detectors/unregistered_receiver.cpp
//!
//! Checks whether ft_transfer / ft_transfer_call allow transfers to
//! unregistered accounts (accounts not in the token's account map).
//! An "allowed" pattern is when the receiver_id flows into a map `get`
//! whose result is then unwrapped without safety checks.
//!
//! Output: $TMP_DIR/.unregistered-receiver.tmp  (format: funcname@True/False)
//! True = unregistered receiver allowed (bad), False = guarded (good).

use llvm_sys::core::{LLVMGetCalledValue, LLVMIsAFunction};
use near_core::{
    ir::{
        is_inst_call_func, simple_find_users, Context, FunctionRef, InstructionRef, Module,
    },
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;

/// True if `receiver` (when used in `func`) flows into an unsafe map lookup
/// (map.get(receiver).unwrap_or / unwrap_unchecked).
/// Mirrors `allowUnregisteredReceiver`.
fn allow_unregistered_receiver(
    func: FunctionRef,
    receiver: llvm_sys::prelude::LLVMValueRef,
    depth: i32,
    visited: &mut HashSet<llvm_sys::prelude::LLVMValueRef>,
) -> bool {
    if depth < 0 {
        return false;
    }
    if !visited.insert(receiver) {
        return false;
    }

    // Use standard implementation — always treated as safe
    if patterns::ft_transfer_call_standard().is_match(func.name()) {
        return true;
    }

    let re_map_get = patterns::map_get();
    let re_unchecked_unwrap = patterns::unchecked_unwrap();

    let mut users_of_receiver: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
    simple_find_users(receiver, &mut users_of_receiver, false, true);

    for &user in &users_of_receiver {
        let user_inst = InstructionRef(user);
        if !user_inst.is_call() {
            continue;
        }

        if is_inst_call_func(user_inst, re_map_get) {
            // Determine return value of `get`
            let return_val_of_get: llvm_sys::prelude::LLVMValueRef = match user_inst.num_args() {
                2 => user_inst.raw(), // returnVal = call get(self, key)
                3 => user_inst.get_arg(0), // call get(returnVal, self, key)
                _ => continue,
            };

            // Find users of the get result; check if any unchecked_unwrap
            let mut users_of_get: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
            simple_find_users(return_val_of_get, &mut users_of_get, false, false);

            for &get_user in &users_of_get {
                let gui = InstructionRef(get_user);
                if gui.is_call() && is_inst_call_func(gui, re_unchecked_unwrap) {
                    return true;
                }
            }
        } else {
            // Check if receiver flows into a callee, recurse
            let mut next_receiver_offset: Option<u32> = None;
            for i in 0..user_inst.num_args() {
                let arg = user_inst.get_arg(i);
                if users_of_receiver.contains(&arg) {
                    next_receiver_offset = Some(i);
                    break;
                }
            }
            if let Some(off) = next_receiver_offset {
                unsafe {
                    let callee = LLVMGetCalledValue(user_inst.raw());
                    if callee.is_null() || LLVMIsAFunction(callee).is_null() {
                        continue;
                    }
                    let callee_func = FunctionRef(callee);
                    if callee_func.param_count() > off {
                        let next_receiver = callee_func.get_param(off);
                        if allow_unregistered_receiver(
                            callee_func,
                            next_receiver,
                            depth - 1,
                            &mut HashSet::new(),
                        ) {
                            return true;
                        }
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
        eprintln!("Usage: unregistered_receiver <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("unregistered-receiver");
    let re_ft_transfer_trait = patterns::ft_transfer_trait();
    let re_ft_transfer_call_trait = patterns::ft_transfer_call_trait();

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

            if re_ft_transfer_trait.is_match(func.name()) {
                if func.param_count() < 3 {
                    continue;
                }
                // receiver_id is arg 1 (self=0, receiver_id=1, amount=2)
                let receiver = func.get_param(1);
                let allowed = allow_unregistered_receiver(func, receiver, 5, &mut HashSet::new());
                if allowed {
                    eprintln!(
                        "\x1b[33m[!] unregistered receiver allowed in ft_transfer {}\x1b[0m",
                        func.name()
                    );
                }
                writer.write_bool(func.name(), !allowed);
            } else if re_ft_transfer_call_trait.is_match(func.name()) {
                if func.param_count() < 3 {
                    continue;
                }
                // receiver_id is arg 2 (self=0, receiver_id=1... wait C++ uses 2)
                let receiver = func.get_param(2);
                let allowed = allow_unregistered_receiver(func, receiver, 5, &mut HashSet::new());
                if allowed {
                    eprintln!(
                        "\x1b[33m[!] unregistered receiver allowed in ft_transfer_call {}\x1b[0m",
                        func.name()
                    );
                }
                writer.write_bool(func.name(), !allowed);
            }
        }
    }
}
