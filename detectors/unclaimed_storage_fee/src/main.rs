//! unclaimed_storage_fee detector — port of detectors/unclaimed_storage_fee.cpp
//!
//! Checks whether `storage_unregister` implementations compare the account
//! balance against zero before allowing withdrawal.
//!
//! Output: $TMP_DIR/.unclaimed-storage-fee.tmp  (format: funcname@True/False)

use llvm_sys::core::{
    LLVMGetCalledValue, LLVMGetIntTypeWidth, LLVMGetOperand, LLVMGetTypeKind, LLVMIsAFunction,
    LLVMIsAICmpInst, LLVMTypeOf,
};
use llvm_sys::{LLVMIntPredicate, LLVMTypeKind};
use near_core::{
    ir::{all_instructions, Context, FunctionRef, Module},
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;

fn is_const_zero(v: llvm_sys::prelude::LLVMValueRef) -> bool {
    unsafe {
        if v.is_null() {
            return false;
        }
        // LLVMIsConstantInt is not directly available; use LLVMConstIntGetZExtValue
        let val = llvm_sys::core::LLVMConstIntGetZExtValue(v);
        val == 0
    }
}

/// True if `func` (or a transitive callee) contains a 128-bit integer comparison
/// against zero — indicating a balance check.
fn has_balance_cmp(
    func: FunctionRef,
    depth: i32,
    visited: &mut HashSet<llvm_sys::prelude::LLVMValueRef>,
) -> bool {
    if depth <= 0 {
        return false;
    }
    if patterns::is_lib_func(func.name()) {
        return false;
    }
    if !visited.insert(func.raw()) {
        return false;
    }

    for inst in all_instructions(func) {
        // Recurse into direct callees
        if inst.is_call() {
            unsafe {
                let callee = LLVMGetCalledValue(inst.raw());
                if !callee.is_null() && !LLVMIsAFunction(callee).is_null() {
                    let callee_func = FunctionRef(callee);
                    if has_balance_cmp(callee_func, depth - 1, visited) {
                        return true;
                    }
                }
            }
        }

        // Check for ICmp instructions
        unsafe {
            if LLVMIsAICmpInst(inst.raw()).is_null() {
                continue;
            }

            let op0 = LLVMGetOperand(inst.raw(), 0);
            let op1 = LLVMGetOperand(inst.raw(), 1);
            if op0.is_null() || op1.is_null() {
                continue;
            }

            // Check if operand 0 is a 128-bit integer
            let ty0 = LLVMTypeOf(op0);
            if ty0.is_null() {
                continue;
            }
            if LLVMGetTypeKind(ty0) != LLVMTypeKind::LLVMIntegerTypeKind
                || LLVMGetIntTypeWidth(ty0) != 128
            {
                continue;
            }

            let pred = llvm_sys::core::LLVMGetICmpPredicate(inst.raw());

            // balance == 0 or balance != 0: either operand is zero constant
            if pred == LLVMIntPredicate::LLVMIntEQ || pred == LLVMIntPredicate::LLVMIntNE {
                if is_const_zero(op1) || is_const_zero(op0) {
                    return true;
                }
            }
            // balance <= 0 (ULE with rhs=0)
            if pred == LLVMIntPredicate::LLVMIntULE && is_const_zero(op1) {
                return true;
            }
            // balance > 0 (UGT with rhs=0)
            if pred == LLVMIntPredicate::LLVMIntUGT && is_const_zero(op1) {
                return true;
            }
            // 0 >= balance (UGE with lhs=0)
            if pred == LLVMIntPredicate::LLVMIntUGE && is_const_zero(op0) {
                return true;
            }
            // 0 < balance (ULT with lhs=0)
            if pred == LLVMIntPredicate::LLVMIntULT && is_const_zero(op0) {
                return true;
            }
        }
    }
    false
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: unclaimed_storage_fee <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("unclaimed-storage-fee");
    let re_storage_unregister = patterns::storage_unregister();

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
            if !re_storage_unregister.is_match(func.name()) {
                continue;
            }

            let mut visited = HashSet::new();
            let ok = has_balance_cmp(func, 3, &mut visited);
            if ok {
                eprintln!(
                    "\x1b[33m[*] unclaimed_storage_fee: balance check present in {}\x1b[0m",
                    func.name()
                );
            } else {
                eprintln!(
                    "\x1b[33m[!] unclaimed_storage_fee: balance check missing in {}\x1b[0m",
                    func.name()
                );
            }
            writer.write_bool(func.name(), ok);
        }
    }
}
