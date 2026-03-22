//! unsaved_changes detector — port of detectors/unsaved_changes.cpp
//!
//! Finds map entries that are fetched, mutated (via StoreInst), but never
//! written back with `insert` — indicating the change will be lost.
//!
//! Output: $TMP_DIR/.unsaved-changes.tmp  (format: funcname@filename@line)

use near_core::{
    ir::{all_instructions, is_inst_call_func, simple_find_users, Context, InstructionRef, Module},
    output::TmpWriter,
    patterns,
};
use std::collections::HashSet;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: unsaved_changes <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("unsaved-changes");
    let re_map_get = patterns::map_get();
    let re_map_insert = patterns::map_insert();
    let re_all_unwrap = patterns::all_unwrap();

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

                if !inst.is_call() || !is_inst_call_func(inst, re_map_get) {
                    continue;
                }

                // Determine where the return value of `get` is stored
                let return_val_of_get: llvm_sys::prelude::LLVMValueRef = match inst.num_args() {
                    2 => inst.raw(), // returnVal = call get(self, key)
                    3 => inst.get_arg(0), // call get(returnVal, self, key)
                    _ => continue,
                };

                // Find all users of the return value (disable cross-fn)
                let mut users_of_get: HashSet<llvm_sys::prelude::LLVMValueRef> = HashSet::new();
                simple_find_users(return_val_of_get, &mut users_of_get, false, true);

                // Find an unwrap call in those users
                let mut unwrapped_value: Option<llvm_sys::prelude::LLVMValueRef> = None;
                for &user in &users_of_get {
                    let user_inst = InstructionRef(user);
                    if user_inst.is_call() && is_inst_call_func(user_inst, re_all_unwrap) {
                        // The unwrapped value is arg[0] of the unwrap call
                        if user_inst.num_args() >= 1 {
                            unwrapped_value = Some(user_inst.get_arg(0));
                        }
                        break;
                    }
                }

                let unwrapped = match unwrapped_value {
                    Some(v) => v,
                    None => continue, // no unwrap — skip
                };

                // Find all users of the unwrapped value (allow cross-fn)
                let mut users_of_unwrapped: HashSet<llvm_sys::prelude::LLVMValueRef> =
                    HashSet::new();
                simple_find_users(unwrapped, &mut users_of_unwrapped, true, false);

                // Check if there's a StoreInst among users (mutation)
                let has_store = users_of_unwrapped
                    .iter()
                    .any(|&u| InstructionRef(u).is_store());

                if !has_store {
                    continue;
                }

                // Check if there's an `insert` call among users (saving back)
                let has_insert = users_of_unwrapped.iter().any(|&u| {
                    let ui = InstructionRef(u);
                    ui.is_call() && is_inst_call_func(ui, re_map_insert)
                });

                if !has_insert {
                    eprintln!(
                        "\x1b[33m[!] unsaved_changes: map mutation not re-inserted in {} @ {}:{}\x1b[0m",
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
