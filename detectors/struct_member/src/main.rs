//! struct_member detector — port of detectors/struct_member.cpp
//!
//! Finds all accesses (read/write) to struct members listed in
//! $TMP_DIR/.structs.tmp. Logs found accesses to stderr.
//!
//! Input:  $TMP_DIR/.structs.tmp  (one struct name per line, # = comment)
//! Output: stderr only (informational, no .tmp file written)

use llvm_sys::core::{
    LLVMDisposeMessage, LLVMGetFirstUse, LLVMGetGEPSourceElementType, LLVMGetInstructionOpcode,
    LLVMGetOperand, LLVMGetUser, LLVMIsAGetElementPtrInst, LLVMIsALoadInst, LLVMIsAStoreInst,
    LLVMPrintTypeToString, LLVMTypeOf,
};
use llvm_sys::prelude::{LLVMTypeRef, LLVMValueRef};
use llvm_sys::LLVMOpcode;
use near_core::{
    ir::{raw_bb_instructions, Context, Module},
    patterns,
};
use std::collections::HashSet;
use std::ffi::CStr;
use std::fs;

fn type_to_string(ty: LLVMTypeRef) -> String {
    unsafe {
        if ty.is_null() {
            return String::new();
        }
        let s = LLVMPrintTypeToString(ty);
        let result = CStr::from_ptr(s).to_string_lossy().into_owned();
        LLVMDisposeMessage(s);
        result
    }
}

fn is_cast_inst(v: LLVMValueRef) -> bool {
    unsafe {
        matches!(
            LLVMGetInstructionOpcode(v),
            LLVMOpcode::LLVMTrunc
                | LLVMOpcode::LLVMZExt
                | LLVMOpcode::LLVMSExt
                | LLVMOpcode::LLVMFPToUI
                | LLVMOpcode::LLVMFPToSI
                | LLVMOpcode::LLVMUIToFP
                | LLVMOpcode::LLVMSIToFP
                | LLVMOpcode::LLVMFPTrunc
                | LLVMOpcode::LLVMFPExt
                | LLVMOpcode::LLVMPtrToInt
                | LLVMOpcode::LLVMIntToPtr
                | LLVMOpcode::LLVMBitCast
                | LLVMOpcode::LLVMAddrSpaceCast
        )
    }
}

#[derive(Debug, PartialEq)]
enum Mode {
    Read,
    Write,
    Unknown,
}

fn get_mode(inst: LLVMValueRef) -> Mode {
    unsafe {
        let use_ref = LLVMGetFirstUse(inst);
        if use_ref.is_null() {
            return Mode::Unknown;
        }
        let user = LLVMGetUser(use_ref);
        if user.is_null() {
            return Mode::Unknown;
        }
        if !LLVMIsALoadInst(user).is_null() {
            Mode::Read
        } else if !LLVMIsAStoreInst(user).is_null() {
            Mode::Write
        } else {
            Mode::Unknown
        }
    }
}

fn using_struct(inst: LLVMValueRef, structs: &HashSet<String>) -> Option<(String, Mode)> {
    unsafe {
        // GEP: for non-zero offsets — source element type identifies the struct
        if !LLVMIsAGetElementPtrInst(inst).is_null() {
            let src_ty = LLVMGetGEPSourceElementType(inst);
            let src_meta = type_to_string(src_ty);
            // src_meta looks like "%\"contract::Contract\" = type { ... }"
            // Extract name: from position 1 up to the '=' (exclusive), strip quotes
            if let Some(eq_pos) = src_meta.find('=') {
                if eq_pos >= 2 {
                    let raw = src_meta[1..eq_pos - 1].trim();
                    let struct_name = raw.trim_matches('"').to_string();
                    if structs.contains(&struct_name) {
                        return Some((struct_name, get_mode(inst)));
                    }
                }
            }
        }

        // CastInst: for zero-offset member (casting struct pointer)
        if is_cast_inst(inst) {
            let src_operand = LLVMGetOperand(inst, 0);
            if src_operand.is_null() {
                return None;
            }
            let src_ty = LLVMTypeOf(src_operand);
            let src_meta = type_to_string(src_ty);
            // src_meta looks like "%\"contract::Contract\"*"
            if src_meta.starts_with('%') && src_meta.ends_with('*') {
                let inner = &src_meta[1..src_meta.len() - 1]; // strip % and *
                if inner.ends_with('*') {
                    return None; // only 1 level of pointer deref
                }
                for name in structs {
                    if src_meta.contains(name.as_str()) && inner == name.as_str() {
                        return Some((name.clone(), get_mode(inst)));
                    }
                }
            }
        }

        None
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: struct_member <bitcode_file> [...]");
        std::process::exit(1);
    }

    // Read struct names from .structs.tmp
    let tmp_dir = std::env::var("TMP_DIR").unwrap_or_else(|_| "./.tmp".to_string());
    let structs_file = format!("{}/.structs.tmp", tmp_dir.trim_end_matches('/'));
    let structs: HashSet<String> = fs::read_to_string(&structs_file)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect();

    if structs.is_empty() {
        eprintln!("warning: no struct names found in {}", structs_file);
    }

    let ctx = Context::new();

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

            for bb in func.basic_blocks() {
                for inst in raw_bb_instructions(bb.raw_bb()) {
                    // Skip instructions without a non-lib debug location
                    let loc = match inst.debug_loc() {
                        Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                        _ => continue,
                    };

                    if let Some((name, mode)) = using_struct(inst.raw(), &structs) {
                        let mode_str = match mode {
                            Mode::Read => "read",
                            Mode::Write => "write",
                            Mode::Unknown => "unknown",
                        };
                        eprintln!(
                            "\x1b[33m[*] struct_member: struct <{}> used({}) at {}:{}\x1b[0m",
                            name, mode_str, loc.filename, loc.line
                        );
                    }
                }
            }
        }
    }
}
