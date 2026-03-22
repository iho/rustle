//! nft_approval_check detector — port of detectors/nft_approval_check.cpp
//!
//! Checks whether NFT transfer implementations verify the `approval_id`
//! argument before executing the transfer.
//!
//! Output: $TMP_DIR/.nft-approval-check.tmp  (format: funcname@True/False)

use llvm_sys::core::{
    LLVMDisposeMessage, LLVMGetCalledValue, LLVMGetGEPSourceElementType, LLVMIsAFunction,
    LLVMIsAGetElementPtrInst, LLVMIsAStoreInst, LLVMPrintTypeToString,
};
use near_core::{
    ir::{
        all_instructions, is_inst_call_func, simple_find_users, Context, FunctionRef,
        InstructionRef, Module,
    },
    output::TmpWriter,
    patterns,
};
use std::ffi::CStr;
use std::collections::HashSet;

/// True if `func` (given `approval_id0_offset` — index of the approval_id.0 param)
/// performs an approval_id equality check.
fn has_approval_check(
    func: FunctionRef,
    approval_id0_offset: u32,
    depth: i32,
    visited: &mut HashSet<llvm_sys::prelude::LLVMValueRef>,
) -> bool {
    if depth < 0 {
        return false;
    }
    if !visited.insert(func.raw()) {
        return false;
    }

    // Use standard NFT implementation — always treated as checked
    if patterns::nft_standard_transfer().is_match(func.name()) {
        return true;
    }

    if func.param_count() <= approval_id0_offset {
        return false;
    }

    let approval_id0 = func.get_param(approval_id0_offset);

    // Find all users of approval_id0 (disable cross-fn)
    let mut users_of_approval_id0: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
    simple_find_users(approval_id0, &mut users_of_approval_id0, false, true);

    // Try to find the actual approval_id Value by looking for StoreInst → GEP
    // with source element type "{ i64, i64 }" (Option<u64> struct).
    let mut approval_id: Option<llvm_sys::prelude::LLVMValueRef> = None;
    for &user in &users_of_approval_id0 {
        unsafe {
            if LLVMIsAStoreInst(user).is_null() {
                continue;
            }
            let store_inst = InstructionRef(user);
            let ptr_op = store_inst.get_operand(1); // pointer operand of store
            if ptr_op.is_null() || LLVMIsAGetElementPtrInst(ptr_op).is_null() {
                continue;
            }
            // Check source element type of the GEP
            let gep_src_ty = LLVMGetGEPSourceElementType(ptr_op);
            let s = LLVMPrintTypeToString(gep_src_ty);
            let src_type_str = CStr::from_ptr(s).to_string_lossy().into_owned();
            LLVMDisposeMessage(s);
            if src_type_str == "{ i64, i64 }" {
                // Treat this GEP's pointer operand as the approvalId Value
                let gep_inst = InstructionRef(ptr_op);
                approval_id = Some(gep_inst.get_operand(0));
                break;
            }
        }
    }

    // Find all users of the resolved approval_id (if we found it via GEP)
    let mut users_of_approval_id: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
    if let Some(aid) = approval_id {
        simple_find_users(aid, &mut users_of_approval_id, false, true);
    }

    let re_partial_eq = patterns::partial_eq();

    for inst in all_instructions(func) {
        if is_inst_call_func(inst, re_partial_eq) {
            // Check if any argument is in usersOfApprovalId or usersOfApprovalId0
            for i in 0..inst.num_args() {
                let arg = inst.get_arg(i);
                if users_of_approval_id.contains(&arg) || users_of_approval_id0.contains(&arg) {
                    return true;
                }
            }
        } else if inst.is_call() {
            // Find if approval_id0 flows into this callee, recurse
            let mut next_offset: Option<u32> = None;
            for i in 0..inst.num_args() {
                if users_of_approval_id0.contains(&inst.get_arg(i)) {
                    next_offset = Some(i);
                    break;
                }
            }
            if let Some(off) = next_offset {
                unsafe {
                    let callee = LLVMGetCalledValue(inst.raw());
                    if !callee.is_null() && !LLVMIsAFunction(callee).is_null() {
                        if has_approval_check(FunctionRef(callee), off, depth - 1, visited) {
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
        eprintln!("Usage: nft_approval_check <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("nft-approval-check");
    let re_nft_transfer = patterns::nft_transfer_bare();
    let re_nft_transfer_call = patterns::nft_transfer_call_bare();

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

            if re_nft_transfer.is_match(func.name()) {
                if func.param_count() < 6 {
                    continue;
                }
                eprintln!("\x1b[33m[*] Find nft_transfer {}\x1b[0m", func.name());
                // near-sdk 5: AccountId = Arc<str> → (ptr, len) = 2 params.
                // nft_transfer(self=0, receiver_id.ptr=1, receiver_id.len=2, token_id=3,
                //              approval_id.0=4, approval_id.1=5, memo=6)
                let ok = has_approval_check(func, 4, 5, &mut HashSet::new());
                writer.write_bool(func.name(), ok);
            } else if re_nft_transfer_call.is_match(func.name()) {
                if func.param_count() < 7 {
                    continue;
                }
                eprintln!(
                    "\x1b[33m[*] Find nft_transfer_call {}\x1b[0m",
                    func.name()
                );
                // nft_transfer_call has sret return + same layout:
                // (sret=0, self=1, receiver_id.ptr=2, receiver_id.len=3, token_id=4,
                //  approval_id.0=5, approval_id.1=6, memo=7, msg=8)
                let ok = has_approval_check(func, 5, 5, &mut HashSet::new());
                writer.write_bool(func.name(), ok);
            }
        }
    }
}
