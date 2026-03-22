//! nft_owner_check detector — port of detectors/nft_owner_check.cpp
//!
//! Checks whether NFT approve/revoke functions perform an owner check
//! (call `predecessor_account_id` and compare with `PartialEq<AccountId>`).
//!
//! Output: $TMP_DIR/.nft-owner-check.tmp  (format: funcname@True/False)

use near_core::{
    ir::{is_func_privileged, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: nft_owner_check <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("nft-owner-check");
    let re_nft_approve = patterns::nft_approve_function();

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
            // Skip closure functions and non-matching names
            if func.name().contains("$closure$") {
                continue;
            }
            if !re_nft_approve.is_match(func.name()) {
                continue;
            }

            let privileged = is_func_privileged(func);
            if privileged {
                eprintln!(
                    "\x1b[33m[*] nft_owner_check: owner check present in {}\x1b[0m",
                    func.name()
                );
            } else {
                eprintln!(
                    "\x1b[33m[!] nft_owner_check: owner check missing in {}\x1b[0m",
                    func.name()
                );
            }
            writer.write_bool(func.name(), privileged);
        }
    }
}
