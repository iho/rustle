//! nep_interface detector — port of detectors/nep_interface.cpp
//!
//! Checks whether the contract implements all required functions for a given
//! NEP standard. The NEP id is specified with --nep-id <id> or -nep-id=<id>.
//!
//! Output: $TMP_DIR/.nep<id>-interface.tmp  (format: unimplemented_func_name)

use near_core::{
    ir::{all_instructions, simple_find_users, Context, InstructionRef, Module},
    output::TmpWriter,
    patterns,
};
use regex::Regex;
use std::collections::{HashMap, HashSet};

// Required functions for each NEP standard.
fn positional_funcs() -> HashMap<u32, Vec<&'static str>> {
    let mut m = HashMap::new();
    m.insert(141, vec!["ft_transfer", "ft_transfer_call", "ft_total_supply", "ft_balance_of"]);
    m.insert(145, vec!["storage_deposit", "storage_withdraw", "storage_unregister", "storage_balance_bounds", "storage_balance_of"]);
    m.insert(148, vec!["ft_metadata"]);
    m.insert(171, vec!["nft_transfer", "nft_transfer_call", "nft_token"]);
    m.insert(177, vec!["nft_metadata"]);
    m.insert(178, vec!["nft_approve", "nft_revoke", "nft_revoke_all", "nft_is_approved"]);
    m.insert(181, vec!["nft_total_supply", "nft_tokens", "nft_supply_for_owner", "nft_tokens_for_owner"]);
    m.insert(199, vec![]);
    m.insert(245, vec!["mt_transfer", "mt_batch_transfer", "mt_transfer_call", "mt_batch_transfer_call",
                       "mt_token", "mt_balance_of", "mt_batch_balance_of", "mt_supply", "mt_batch_supply"]);
    m.insert(297, vec![]);
    m.insert(330, vec![]);
    m.insert(366, vec![]);
    m
}

fn parse_nep_id(args: &[String]) -> Option<u32> {
    for (i, arg) in args.iter().enumerate() {
        // --nep-id=141 or -nep-id=141
        if let Some(val) = arg.strip_prefix("--nep-id=").or_else(|| arg.strip_prefix("-nep-id=")) {
            return val.parse().ok();
        }
        // --nep-id 141 or -nep-id 141
        if (arg == "--nep-id" || arg == "-nep-id") && i + 1 < args.len() {
            return args[i + 1].parse().ok();
        }
    }
    None
}

fn main() {
    let all_args: Vec<String> = std::env::args().collect();
    let nep_id = match parse_nep_id(&all_args) {
        Some(id) => id,
        None => {
            eprintln!("Usage: nep_interface --nep-id <id> <bitcode_file> [...]");
            std::process::exit(1);
        }
    };

    // Collect bitcode files (skip program name and nep-id args)
    let bc_files: Vec<&str> = all_args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-') && !a.parse::<u32>().is_ok())
        .map(|s| s.as_str())
        .collect();

    if bc_files.is_empty() {
        eprintln!("No bitcode files provided.");
        std::process::exit(1);
    }

    let pos_funcs = positional_funcs();
    let required = match pos_funcs.get(&nep_id) {
        Some(r) => r,
        None => {
            eprintln!("Invalid nep-id: {}", nep_id);
            std::process::exit(1);
        }
    };

    let writer = TmpWriter::new(&format!("nep{}-interface", nep_id));
    let ctx = Context::new();

    // Collect all function names from all modules
    let mut all_func_names: HashSet<String> = HashSet::new();
    let mut modules = Vec::new();

    for path in &bc_files {
        match Module::from_bitcode(&ctx, path) {
            Ok(m) => modules.push(m),
            Err(e) => { eprintln!("warning: {e}"); continue; }
        }
    }

    for module in &modules {
        for func in module.functions() {
            if !patterns::is_lib_func(func.name()) {
                all_func_names.insert(func.name().to_string());
            }
        }
    }

    // Check positional functions
    for &func_name in required {
        let pat = format!(r"[0-9]+{}[0-9]+", func_name);
        let re = Regex::new(&pat).unwrap();
        let implemented = all_func_names.iter().any(|f| f == func_name || re.is_match(f));
        if implemented {
            eprintln!("\x1b[33m[*] Implemented: {}\x1b[0m", func_name);
        } else {
            eprintln!("\x1b[33m[!] Unimplemented: {}\x1b[0m", func_name);
            writer.write_name(func_name);
        }
    }

    // Check resolver for transfer-call NEPs (141, 171, 245)
    let neps_with_resolver: HashSet<u32> = [141, 171, 245].into();
    if !neps_with_resolver.contains(&nep_id) {
        return;
    }

    let (transfer_call_re, on_transfer_re, resolver_label) = match nep_id {
        141 => (
            Regex::new(r"[0-9]+ft_transfer_call[0-9]+").unwrap(),
            Regex::new(r"[0-9]+ft_on_transfer[0-9]+").unwrap(),
            "resolver of ft_transfer_call",
        ),
        171 => (
            Regex::new(r"[0-9]+nft_transfer_call[0-9]+").unwrap(),
            Regex::new(r"[0-9]+nft_on_transfer[0-9]+").unwrap(),
            "resolver of nft_transfer_call",
        ),
        245 => (
            Regex::new(r"[0-9]+mt_transfer_call[0-9]+").unwrap(),
            Regex::new(r"[0-9]+mt_on_transfer[0-9]+").unwrap(),
            "resolver of mt_transfer_call",
        ),
        _ => return,
    };
    let re_then = Regex::new(r"near_sdk[0-9]+promise[0-9]+Promise[0-9]+then").unwrap();

    // Mirror the C++ logic: only report unimplemented when a transfer_call function
    // actually exists in the module. If the function doesn't exist at all, skip silently.
    for module in &modules {
        for func in module.functions() {
            if !transfer_call_re.is_match(func.name()) {
                continue;
            }

            // This function IS a transfer_call — now look for the resolver inside it.
            let mut found_resolver = false;
            'resolver: for inst in all_instructions(func) {
                if let Some(callee_name) = inst.called_fn_name() {
                    if !on_transfer_re.is_match(callee_name) {
                        continue;
                    }
                    if inst.num_args() == 0 {
                        continue;
                    }
                    let on_transfer_arg0 = inst.get_arg(0);
                    let mut on_transfer_users: HashSet<llvm_sys::prelude::LLVMValueRef> =
                        HashSet::new();
                    simple_find_users(on_transfer_arg0, &mut on_transfer_users, false, false);

                    for &user in &on_transfer_users {
                        let ui = InstructionRef(user);
                        if ui.is_call() && re_then.is_match(ui.called_fn_name().unwrap_or("")) {
                            if ui.num_args() > 0 {
                                let last_arg = ui.get_arg(ui.num_args() - 1);
                                for resolve_user in near_core::ir::value_users(last_arg) {
                                    let rui = InstructionRef(resolve_user);
                                    if rui.is_call() && resolve_user != user {
                                        found_resolver = true;
                                        break 'resolver;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if found_resolver {
                eprintln!("\x1b[33m[*] Implemented: {}\x1b[0m", resolver_label);
            } else {
                eprintln!("\x1b[33m[!] Unimplemented: {}\x1b[0m", resolver_label);
                writer.write_name(resolver_label);
            }
            return; // only check the first matching transfer_call function
        }
    }
}
