use near_core::{
    ir::{all_instructions, is_func_privileged, is_inst_call_func, Context, Module},
    output::TmpWriter,
    patterns,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: missing_owner_check <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("missing-owner-check");
    let re_transfer = patterns::promise_transfer();
    let re_deploy = patterns::promise_batch_deploy();

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
            if func.name().contains("$closure$") {
                continue;
            }

            // Collect sensitive call locations (transfer or deploy).
            let mut sensitive_loc: Option<(String, u32)> = None;

            for inst in all_instructions(func) {
                let loc = match inst.debug_loc() {
                    Some(l) if !patterns::is_lib_loc(&l.filename) => l,
                    _ => continue,
                };

                if is_inst_call_func(inst, re_transfer) || is_inst_call_func(inst, re_deploy) {
                    if sensitive_loc.is_none() {
                        sensitive_loc = Some((loc.filename.clone(), loc.line));
                    }
                    break;
                }
            }

            let (filename, line) = match sensitive_loc {
                Some(v) => v,
                None => continue,
            };

            let privileged = is_func_privileged(func);
            if privileged {
                eprintln!(
                    "\x1b[33m[*] missing_owner_check: owner check present in {}\x1b[0m",
                    func.name()
                );
            } else {
                eprintln!(
                    "\x1b[33m[!] missing_owner_check: owner check missing in {} @ {}:{}\x1b[0m",
                    func.name(),
                    filename,
                    line
                );
                writer.write(func.name(), &filename, line);
            }
        }
    }
}
