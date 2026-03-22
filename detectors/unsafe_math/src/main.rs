//! unsafe_math detector — port of detectors/unsafe_math.cpp
//!
//! Finds unchecked integer arithmetic (add, sub, mul) instructions that
//! operate on user-defined variables (i.e. values with non-empty names).
//! Integer float variants (fadd, fsub, fmul) are excluded.
//!
//! Output: $TMP_DIR/.unsafe-math.tmp  (format: funcname@filename@line)

use llvm_sys::core::{LLVMGetNumOperands, LLVMGetOperand, LLVMGetValueName2};
use llvm_sys::LLVMOpcode;
use near_core::{
    ir::{all_instructions, Context, InstructionRef, Module},
    output::TmpWriter,
    patterns,
};

/// True if the instruction or any of its operands has a user-defined name.
fn has_named_var(inst: InstructionRef) -> bool {
    unsafe {
        let mut len = 0usize;
        let ptr = LLVMGetValueName2(inst.raw(), &mut len);
        if len > 0 && !ptr.is_null() {
            return true;
        }
        let n = LLVMGetNumOperands(inst.raw());
        for i in 0..n {
            let op = LLVMGetOperand(inst.raw(), i as u32);
            if !op.is_null() {
                let mut len2 = 0usize;
                let ptr2 = LLVMGetValueName2(op, &mut len2);
                if len2 > 0 && !ptr2.is_null() {
                    return true;
                }
            }
        }
        false
    }
}

/// True if the opcode is an integer (non-float) add, sub, or mul.
fn is_integer_arith(inst: InstructionRef) -> bool {
    matches!(
        inst.opcode(),
        LLVMOpcode::LLVMAdd | LLVMOpcode::LLVMSub | LLVMOpcode::LLVMMul
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: unsafe_math <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("unsafe-math");

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

            for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                    _ => continue,
                };

                if is_integer_arith(inst) && has_named_var(inst) {
                    eprintln!(
                        "\x1b[33m[!] unsafe math in {} @ {}:{}\x1b[0m",
                        func.name(),
                        loc.filename,
                        loc.line
                    );
                    writer.write(func.name(), &loc.filename, loc.line);
                }
            }
        }
    }
}
