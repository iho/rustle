//! complex_loop detector — port of detectors/complex_loop.cpp (call_inside_loop.cpp)
//!
//! Finds loops whose weighted instruction count exceeds a threshold.
//! Uses DFS-based back-edge detection to identify loop headers, then counts
//! instructions in loop body blocks (recursively into called functions).
//!
//! Output: $TMP_DIR/.complex-loop.tmp  (format: funcname@filename@line)

use llvm_sys::core::{LLVMGetCalledValue, LLVMIsAFunction};
use llvm_sys::prelude::LLVMBasicBlockRef;
use near_core::{
    ir::{raw_bb_instructions, Context, FunctionRef, Module},
    output::TmpWriter,
    patterns,
};
use std::collections::{HashMap, HashSet};

const MIN_INST_NUM_FOR_LOOP: i32 = 100;

/// Count all non-call instructions in `func`, recursing into direct callees
/// up to `depth` levels.
fn count_func_instructions(
    func: FunctionRef,
    depth: u8,
    visited: &mut HashSet<*mut llvm_sys::LLVMValue>,
) -> i32 {
    if depth == 0 || patterns::is_lib_func(func.name()) {
        return 0;
    }
    if !visited.insert(func.raw() as *mut _) {
        return 0;
    }
    let mut count = 0i32;
    for bb in func.basic_blocks() {
        for inst in bb.instructions() {
            if inst.is_call() {
                unsafe {
                    let callee = LLVMGetCalledValue(inst.raw());
                    if !callee.is_null() && !LLVMIsAFunction(callee).is_null() {
                        count +=
                            count_func_instructions(FunctionRef(callee), depth - 1, visited);
                    }
                }
            } else {
                count += 1;
            }
        }
    }
    count
}

/// Count weighted instructions in a set of basic blocks (the loop body).
fn count_loop_instructions(
    loop_body: &[LLVMBasicBlockRef],
    visited: &mut HashSet<*mut llvm_sys::LLVMValue>,
) -> i32 {
    let mut count = 0i32;
    for &bb in loop_body {
        for inst in raw_bb_instructions(bb) {
            if inst.is_call() {
                unsafe {
                    let callee = LLVMGetCalledValue(inst.raw());
                    if !callee.is_null() && !LLVMIsAFunction(callee).is_null() {
                        count += count_func_instructions(FunctionRef(callee), 2, visited);
                    }
                }
            } else {
                count += 1;
            }
        }
    }
    count
}

/// DFS state for back-edge detection.
struct LoopDetector {
    /// 0 = unvisited, 1 = on stack (gray), 2 = done (black)
    color: HashMap<LLVMBasicBlockRef, u8>,
    stack: Vec<LLVMBasicBlockRef>,
    /// (loop_header, loop_body_bbs)
    loops: Vec<(LLVMBasicBlockRef, Vec<LLVMBasicBlockRef>)>,
}

impl LoopDetector {
    fn new() -> Self {
        LoopDetector {
            color: HashMap::new(),
            stack: Vec::new(),
            loops: Vec::new(),
        }
    }

    fn dfs(&mut self, bb: LLVMBasicBlockRef) {
        self.color.insert(bb, 1);
        self.stack.push(bb);

        // Get successors via the terminator instruction
        if let Some(term) = near_core::ir::BasicBlockRef(bb).terminator() {
            for i in 0..term.num_successors() {
                let succ = term.get_successor(i);
                if succ.is_null() {
                    continue;
                }
                match self.color.get(&succ).copied().unwrap_or(0) {
                    1 => {
                        // Back edge → loop found; succ is the loop header
                        if let Some(pos) = self.stack.iter().position(|&b| b == succ) {
                            let loop_body: Vec<LLVMBasicBlockRef> =
                                self.stack[pos..].to_vec();
                            self.loops.push((succ, loop_body));
                        }
                    }
                    0 => self.dfs(succ),
                    _ => {} // black, skip
                }
            }
        }

        self.color.insert(bb, 2);
        self.stack.pop();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: complex_loop <bitcode_file> [...]");
        std::process::exit(1);
    }

    let ctx = Context::new();
    let writer = TmpWriter::new("complex-loop");

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

            // Get the entry basic block
            let mut bbs = func.basic_blocks();
            let entry_bb = match bbs.next() {
                Some(bb) => bb.raw_bb(),
                None => continue,
            };
            drop(bbs);

            // DFS-based loop detection
            let mut detector = LoopDetector::new();
            detector.dfs(entry_bb);

            for (loop_header, loop_body) in &detector.loops {
                let mut visited: HashSet<*mut llvm_sys::LLVMValue> = HashSet::new();
                let inst_count = count_loop_instructions(loop_body, &mut visited);

                if inst_count > MIN_INST_NUM_FOR_LOOP {
                    // Find debug location of loop header's first instruction
                    let mut loop_filename = String::new();
                    let mut loop_line = 0u32;

                    for inst in raw_bb_instructions(*loop_header) {
                        if let Some(loc) = inst.debug_loc() {
                            if !patterns::is_lib_loc(&loc.filename) {
                                loop_filename = loc.filename;
                                loop_line = loc.line;
                                break;
                            }
                        }
                    }

                    if !loop_filename.is_empty() {
                        eprintln!(
                            "\x1b[33m[!] complex_loop: loop with {} instructions in {} @ {}:{}\x1b[0m",
                            inst_count,
                            func.name(),
                            loop_filename,
                            loop_line
                        );
                        writer.write(func.name(), &loop_filename, loop_line);
                    }
                }
            }
        }
    }
}
