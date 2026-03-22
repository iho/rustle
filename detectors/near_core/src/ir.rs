//! Safe wrappers around the LLVM C API for bitcode analysis.
//!
//! Provides RAII types for Context and Module, plus iterator-based access to
//! functions, basic blocks, instructions, debug locations and value users —
//! mirroring what the C++ detectors consume via FunctionPass / ModulePass.

use llvm_sys::bit_reader::LLVMParseBitcodeInContext2;
use llvm_sys::core::*;
use llvm_sys::debuginfo::{
    LLVMDIFileGetFilename, LLVMDILocationGetLine, LLVMDILocationGetScope, LLVMDIScopeGetFile,
    LLVMInstructionGetDebugLoc,
};
use llvm_sys::prelude::*;
use llvm_sys::{LLVMOpcode, LLVMTypeKind};
use std::collections::HashSet;
use std::ffi::CString;
use std::ptr;

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

pub struct Context(LLVMContextRef);

impl Context {
    pub fn new() -> Self {
        Context(unsafe { LLVMContextCreate() })
    }
    pub(crate) fn raw(&self) -> LLVMContextRef {
        self.0
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { LLVMContextDispose(self.0) }
    }
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

pub struct Module(LLVMModuleRef);

impl Module {
    pub fn from_bitcode(ctx: &Context, path: &str) -> Result<Self, String> {
        let c_path = CString::new(path).map_err(|e| e.to_string())?;
        unsafe {
            let mut mem_buf: LLVMMemoryBufferRef = ptr::null_mut();
            let mut err_msg: *mut libc::c_char = ptr::null_mut();
            if LLVMCreateMemoryBufferWithContentsOfFile(
                c_path.as_ptr(),
                &mut mem_buf,
                &mut err_msg,
            ) != 0
            {
                let msg = if err_msg.is_null() {
                    "unknown error".into()
                } else {
                    let s = std::ffi::CStr::from_ptr(err_msg)
                        .to_string_lossy()
                        .into_owned();
                    LLVMDisposeMessage(err_msg);
                    s
                };
                return Err(format!("cannot read '{}': {}", path, msg));
            }
            let mut module: LLVMModuleRef = ptr::null_mut();
            let failed = LLVMParseBitcodeInContext2(ctx.raw(), mem_buf, &mut module);
            LLVMDisposeMemoryBuffer(mem_buf);
            if failed != 0 || module.is_null() {
                return Err(format!("cannot parse bitcode in '{}'", path));
            }
            Ok(Module(module))
        }
    }

    pub fn functions(&self) -> FunctionIter {
        FunctionIter(unsafe { LLVMGetFirstFunction(self.0) })
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        unsafe { LLVMDisposeModule(self.0) }
    }
}

// ---------------------------------------------------------------------------
// Function iterator & ref
// ---------------------------------------------------------------------------

pub struct FunctionIter(LLVMValueRef);

impl Iterator for FunctionIter {
    type Item = FunctionRef;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_null() {
            return None;
        }
        let cur = self.0;
        self.0 = unsafe { LLVMGetNextFunction(cur) };
        Some(FunctionRef(cur))
    }
}

#[derive(Clone, Copy)]
pub struct FunctionRef(pub LLVMValueRef);

impl FunctionRef {
    pub fn name(&self) -> &str {
        raw_value_name(self.0)
    }
    pub fn basic_blocks(&self) -> BasicBlockIter {
        BasicBlockIter(unsafe { LLVMGetFirstBasicBlock(self.0) })
    }
    pub fn param_count(&self) -> u32 {
        unsafe { LLVMCountParams(self.0) }
    }
    /// Get the i-th parameter value (for use-tracking across function boundaries).
    pub fn get_param(&self, i: u32) -> LLVMValueRef {
        unsafe { LLVMGetParam(self.0, i) }
    }
    pub fn raw(&self) -> LLVMValueRef {
        self.0
    }
}

// ---------------------------------------------------------------------------
// BasicBlock iterator & ref
// ---------------------------------------------------------------------------

pub struct BasicBlockIter(LLVMBasicBlockRef);

impl Iterator for BasicBlockIter {
    type Item = BasicBlockRef;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_null() {
            return None;
        }
        let cur = self.0;
        self.0 = unsafe { LLVMGetNextBasicBlock(cur) };
        Some(BasicBlockRef(cur))
    }
}

#[derive(Clone, Copy)]
pub struct BasicBlockRef(pub LLVMBasicBlockRef);

impl BasicBlockRef {
    pub fn instructions(&self) -> InstructionIter {
        InstructionIter(unsafe { LLVMGetFirstInstruction(self.0) })
    }

    /// Returns the name of this basic block (e.g. "bb", "panic", "return").
    pub fn name(&self) -> &'static str {
        unsafe {
            let ptr = LLVMGetBasicBlockName(self.0);
            if ptr.is_null() {
                return "";
            }
            std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or("")
        }
    }

    pub fn raw_bb(&self) -> LLVMBasicBlockRef {
        self.0
    }

    /// Returns the terminator instruction of this basic block (if any).
    pub fn terminator(&self) -> Option<InstructionRef> {
        unsafe {
            let t = LLVMGetBasicBlockTerminator(self.0);
            if t.is_null() {
                None
            } else {
                Some(InstructionRef(t))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Instruction iterator & ref
// ---------------------------------------------------------------------------

pub struct InstructionIter(LLVMValueRef);

impl Iterator for InstructionIter {
    type Item = InstructionRef;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_null() {
            return None;
        }
        let cur = self.0;
        self.0 = unsafe { LLVMGetNextInstruction(cur) };
        Some(InstructionRef(cur))
    }
}

#[derive(Clone, Copy)]
pub struct InstructionRef(pub LLVMValueRef);

/// Source location from LLVM debug metadata.
pub struct DebugLoc {
    pub filename: String,
    pub line: u32,
}

impl InstructionRef {
    pub fn debug_loc(&self) -> Option<DebugLoc> {
        unsafe {
            let loc = LLVMInstructionGetDebugLoc(self.0);
            if loc.is_null() {
                return None;
            }
            let line = LLVMDILocationGetLine(loc);
            let scope = LLVMDILocationGetScope(loc);
            if scope.is_null() {
                return None;
            }
            let file = LLVMDIScopeGetFile(scope);
            if file.is_null() {
                return None;
            }
            let mut name_len: libc::c_uint = 0;
            let name_ptr = LLVMDIFileGetFilename(file, &mut name_len);
            if name_ptr.is_null() {
                return None;
            }
            let filename = std::str::from_utf8(std::slice::from_raw_parts(
                name_ptr as *const u8,
                name_len as usize,
            ))
            .unwrap_or("")
            .to_owned();
            Some(DebugLoc { filename, line })
        }
    }

    /// If this is a direct function call/invoke, returns the callee's mangled name.
    pub fn called_fn_name(&self) -> Option<&str> {
        unsafe {
            if LLVMIsACallInst(self.0).is_null() && LLVMIsAInvokeInst(self.0).is_null() {
                return None;
            }
            let callee = LLVMGetCalledValue(self.0);
            if callee.is_null() || LLVMIsAFunction(callee).is_null() {
                return None;
            }
            let s = raw_value_name(callee);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }

    pub fn is_call(&self) -> bool {
        unsafe { !LLVMIsACallInst(self.0).is_null() || !LLVMIsAInvokeInst(self.0).is_null() }
    }

    /// Number of call-argument operands (excludes the callee operand).
    pub fn num_args(&self) -> u32 {
        unsafe { LLVMGetNumArgOperands(self.0) }
    }

    /// Get the i-th argument operand of a call instruction.
    pub fn get_arg(&self, i: u32) -> LLVMValueRef {
        unsafe { LLVMGetArgOperand(self.0, i) }
    }

    /// The opcode of this instruction.
    pub fn opcode(&self) -> LLVMOpcode {
        unsafe { LLVMGetInstructionOpcode(self.0) }
    }

    /// True if this is an integer or float multiplication instruction.
    pub fn is_mul(&self) -> bool {
        matches!(self.opcode(), LLVMOpcode::LLVMMul | LLVMOpcode::LLVMFMul)
    }

    /// True if this is an integer or float division instruction.
    pub fn is_div(&self) -> bool {
        matches!(
            self.opcode(),
            LLVMOpcode::LLVMUDiv | LLVMOpcode::LLVMSDiv | LLVMOpcode::LLVMFDiv
        )
    }

    /// True if this calls an LLVM `*.mul.with.overflow.*` intrinsic.
    pub fn is_llvm_mul_overflow(&self) -> bool {
        self.called_fn_name()
            .map(|n| {
                // matches Regex("llvm\.[a-z]?mul\.with\.overflow\.")
                let re = crate::patterns::llvm_mul_overflow();
                re.is_match(n)
            })
            .unwrap_or(false)
    }

    /// Print the LLVM type of the i-th argument as a string.
    /// Used for type-name–based checks (e.g. AccountId detection in self_transfer).
    pub fn arg_type_string(&self, i: u32) -> String {
        unsafe {
            let v = LLVMGetArgOperand(self.0, i);
            value_type_string(v)
        }
    }

    pub fn raw(&self) -> LLVMValueRef {
        self.0
    }

    pub fn is_store(&self) -> bool {
        unsafe { !LLVMIsAStoreInst(self.0).is_null() }
    }

    pub fn is_switch(&self) -> bool {
        unsafe { !LLVMIsASwitchInst(self.0).is_null() }
    }

    pub fn is_branch(&self) -> bool {
        unsafe { !LLVMIsABranchInst(self.0).is_null() }
    }

    pub fn is_return(&self) -> bool {
        unsafe { !LLVMIsAReturnInst(self.0).is_null() }
    }

    pub fn num_operands(&self) -> u32 {
        unsafe { LLVMGetNumOperands(self.0) as u32 }
    }

    pub fn get_operand(&self, i: u32) -> LLVMValueRef {
        unsafe { LLVMGetOperand(self.0, i) }
    }

    /// Number of successors for a terminator instruction (branch, switch, …).
    pub fn num_successors(&self) -> u32 {
        unsafe { LLVMGetNumSuccessors(self.0) }
    }

    /// Get the i-th successor basic block of a terminator instruction.
    pub fn get_successor(&self, i: u32) -> LLVMBasicBlockRef {
        unsafe { LLVMGetSuccessor(self.0, i) }
    }
}

// ---------------------------------------------------------------------------
// Value-level helpers (raw LLVMValueRef, for use in analysis functions)
// ---------------------------------------------------------------------------

/// Return the mangled name of a Value (function or global).
pub fn raw_value_name(v: LLVMValueRef) -> &'static str {
    unsafe {
        let mut len = 0usize;
        let ptr = LLVMGetValueName2(v, &mut len);
        if ptr.is_null() || len == 0 {
            return "";
        }
        // SAFETY: LLVM owns the storage for the name; it lives as long as the
        // module, so 'static is a safe approximation for our analysis lifetime.
        std::str::from_utf8(std::slice::from_raw_parts(ptr as *const u8, len)).unwrap_or("")
    }
}

/// Print the full LLVM IR representation of `v` to a String.
/// Useful for type-name checks embedded in the value's printed form.
pub fn print_value_to_string(v: LLVMValueRef) -> String {
    unsafe {
        if v.is_null() {
            return String::new();
        }
        let s = LLVMPrintValueToString(v);
        let result = std::ffi::CStr::from_ptr(s).to_string_lossy().into_owned();
        LLVMDisposeMessage(s);
        result
    }
}

/// Print the LLVM type of `v` to a String (e.g. `%"near_sdk::…::AccountId"`).
pub fn value_type_string(v: LLVMValueRef) -> String {
    unsafe {
        let ty = LLVMTypeOf(v);
        if ty.is_null() {
            return String::new();
        }
        let s = LLVMPrintTypeToString(ty);
        let result = std::ffi::CStr::from_ptr(s).to_string_lossy().into_owned();
        LLVMDisposeMessage(s);
        result
    }
}

// ---------------------------------------------------------------------------
// Use / user iteration
// ---------------------------------------------------------------------------

/// Iterator over all users of an `LLVMValueRef` (equivalent to `value->users()`).
pub struct UseIter(LLVMUseRef);

impl Iterator for UseIter {
    type Item = LLVMValueRef;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_null() {
            return None;
        }
        let cur = self.0;
        let user = unsafe { LLVMGetUser(cur) };
        self.0 = unsafe { LLVMGetNextUse(cur) };
        Some(user)
    }
}

pub fn value_users(v: LLVMValueRef) -> UseIter {
    UseIter(unsafe { LLVMGetFirstUse(v) })
}

// ---------------------------------------------------------------------------
// simple_find_users  (port of Rustle::simpleFindUsers from near_core.cpp)
// ---------------------------------------------------------------------------

/// Transitively collect all Values that use `value`, following call arguments
/// into callees (cross-function) and StoreInst pointer operands.
///
/// - `restrict_cross_fn`: only follow a callee parameter when the matching call
///   argument is already in `set`.
/// - `disable_cross_fn`: never follow into callees.
///
/// Mirrors the semantics of `Rustle::simpleFindUsers` exactly.
pub fn simple_find_users(
    value: LLVMValueRef,
    set: &mut HashSet<LLVMValueRef>,
    restrict_cross_fn: bool,
    disable_cross_fn: bool,
) {
    unsafe { simple_find_users_inner(value, set, restrict_cross_fn, disable_cross_fn) }
}

unsafe fn simple_find_users_inner(
    value: LLVMValueRef,
    set: &mut HashSet<LLVMValueRef>,
    restrict_cross_fn: bool,
    disable_cross_fn: bool,
) {
    if value.is_null() {
        return;
    }
    let vtype = LLVMTypeOf(value);
    if vtype.is_null() {
        return;
    }
    if LLVMGetTypeKind(vtype) == LLVMTypeKind::LLVMLabelTypeKind {
        return;
    }
    if !set.insert(value) {
        return; // already visited
    }

    let is_call = !LLVMIsACallInst(value).is_null() || !LLVMIsAInvokeInst(value).is_null();
    if is_call {
        let callee = LLVMGetCalledValue(value);
        let is_direct_fn = !callee.is_null() && !LLVMIsAFunction(callee).is_null();
        let n = LLVMGetNumArgOperands(value);

        if !disable_cross_fn && is_direct_fn {
            let param_count = LLVMCountParams(callee);
            for i in 0..n {
                let arg_op = LLVMGetArgOperand(value, i);
                if !restrict_cross_fn || set.contains(&arg_op) {
                    if i < param_count {
                        let param = LLVMGetParam(callee, i);
                        simple_find_users_inner(param, set, restrict_cross_fn, disable_cross_fn);
                    }
                }
            }
            // Special case: `xxx.into()` — add the source arg as a user of arg[1]
            let name = raw_value_name(callee);
            if name.contains("core..convert..Into") && n >= 2 {
                let arg1 = LLVMGetArgOperand(value, 1);
                if set.contains(&arg1) {
                    let arg0 = LLVMGetArgOperand(value, 0);
                    simple_find_users_inner(arg0, set, restrict_cross_fn, disable_cross_fn);
                }
            }
        }
    } else if !LLVMIsAStoreInst(value).is_null() {
        // operand 0 = stored value, operand 1 = pointer destination
        let ptr = LLVMGetOperand(value, 1);
        simple_find_users_inner(ptr, set, restrict_cross_fn, disable_cross_fn);
    }

    // Follow all direct Value users
    let mut use_ref = LLVMGetFirstUse(value);
    while !use_ref.is_null() {
        let user = LLVMGetUser(use_ref);
        simple_find_users_inner(user, set, restrict_cross_fn, disable_cross_fn);
        use_ref = LLVMGetNextUse(use_ref);
    }
}

// ---------------------------------------------------------------------------
// Convenience helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Call-graph helpers (no LLVM CallGraph API — manual callee traversal)
// ---------------------------------------------------------------------------

/// True if `inst` calls anything (transitively) matching `regex`.
/// Mirrors `Rustle::isInstCallFuncRec`.
pub fn is_inst_call_func_rec(inst: InstructionRef, regex: &regex::Regex) -> bool {
    if is_inst_call_func(inst, regex) {
        return true;
    }
    if !inst.is_call() {
        return false;
    }
    unsafe {
        let callee = LLVMGetCalledValue(inst.raw());
        if callee.is_null() || LLVMIsAFunction(callee).is_null() {
            return false;
        }
        let name = raw_value_name(callee);
        if name.starts_with("llvm") {
            return false;
        }
        let mut visited = std::collections::HashSet::new();
        _func_calls_func_rec(FunctionRef(callee), regex, &mut visited)
    }
}

/// True if `func` (transitively) calls anything matching `regex`.
/// Mirrors `Rustle::isFuncCallFuncRec`.
pub fn func_calls_func_rec(func: FunctionRef, regex: &regex::Regex) -> bool {
    let name = func.name();
    if name.starts_with("llvm") || name.is_empty() {
        return false;
    }
    let mut visited = std::collections::HashSet::new();
    _func_calls_func_rec(func, regex, &mut visited)
}

fn _func_calls_func_rec(
    func: FunctionRef,
    regex: &regex::Regex,
    visited: &mut std::collections::HashSet<LLVMValueRef>,
) -> bool {
    if !visited.insert(func.raw()) {
        return false;
    }
    for bb in func.basic_blocks() {
        for inst in bb.instructions() {
            if let Some(name) = inst.called_fn_name() {
                if regex.is_match(name) {
                    return true;
                }
                if !name.starts_with("llvm") {
                    unsafe {
                        let callee = LLVMGetCalledValue(inst.raw());
                        if !callee.is_null() && !LLVMIsAFunction(callee).is_null() {
                            if _func_calls_func_rec(FunctionRef(callee), regex, visited) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// Collect all caller functions of `func`, up to `depth` levels up.
/// Returns a set of raw `LLVMValueRef` (each pointing to a function).
/// Mirrors `Rustle::findFunctionCallerRec`.
pub fn find_function_callers(
    func: FunctionRef,
    depth: u32,
) -> std::collections::HashSet<LLVMValueRef> {
    let mut set = std::collections::HashSet::new();
    _find_function_callers_rec(func, &mut set, depth);
    set
}

fn _find_function_callers_rec(
    func: FunctionRef,
    set: &mut std::collections::HashSet<LLVMValueRef>,
    depth: u32,
) {
    if depth == 0 {
        return;
    }
    for user in value_users(func.raw()) {
        if user.is_null() {
            continue;
        }
        unsafe {
            let caller_bb = LLVMGetInstructionParent(user);
            if caller_bb.is_null() {
                continue;
            }
            let caller_fn = LLVMGetBasicBlockParent(caller_bb);
            if caller_fn.is_null() {
                continue;
            }
            if set.insert(caller_fn) {
                _find_function_callers_rec(FunctionRef(caller_fn), set, depth - 1);
            }
        }
    }
}

/// True if `func` performs an owner/access check — i.e. it calls
/// `predecessor_account_id` and `AccountId::eq` (directly or transitively).
/// Mirrors `Rustle::isFuncPrivileged`.
pub fn is_func_privileged(func: FunctionRef) -> bool {
    _is_func_privileged_inner(func, 5, &mut std::collections::HashSet::new())
}

fn _is_func_privileged_inner(
    func: FunctionRef,
    depth: i32,
    visited: &mut std::collections::HashSet<LLVMValueRef>,
) -> bool {
    if depth < 0 {
        return false;
    }
    if !visited.insert(func.raw()) {
        return false;
    }
    let re_pred = crate::patterns::predecessor_account_id();
    let re_eq = crate::patterns::account_id_eq();

    for inst in all_instructions(func) {
        // Skip instructions without non-lib debug locations
        match inst.debug_loc() {
            Some(loc) if !crate::patterns::is_lib_loc(&loc.filename) => {}
            _ => continue,
        }

        if is_inst_call_func(inst, re_pred) {
            // Check if any instruction in F calls PartialEq for AccountId
            for inst2 in all_instructions(func) {
                if is_inst_call_func(inst2, re_eq) {
                    return true;
                }
            }
        } else if inst.is_call() {
            unsafe {
                let callee = LLVMGetCalledValue(inst.raw());
                if callee.is_null() || LLVMIsAFunction(callee).is_null() {
                    continue;
                }
                if _is_func_privileged_inner(FunctionRef(callee), depth - 1, visited) {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Convenience helpers
// ---------------------------------------------------------------------------

/// Iterate every instruction in every basic block of a function.
pub fn all_instructions(func: FunctionRef) -> impl Iterator<Item = InstructionRef> {
    func.basic_blocks().flat_map(|bb| bb.instructions())
}

/// Iterate every instruction in a raw basic block reference.
pub fn raw_bb_instructions(bb: LLVMBasicBlockRef) -> RawBBInstructionIter {
    RawBBInstructionIter(unsafe { LLVMGetFirstInstruction(bb) })
}

pub struct RawBBInstructionIter(LLVMValueRef);

impl Iterator for RawBBInstructionIter {
    type Item = InstructionRef;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_null() {
            return None;
        }
        let cur = self.0;
        self.0 = unsafe { LLVMGetNextInstruction(cur) };
        Some(InstructionRef(cur))
    }
}

/// True if the instruction calls a function whose name matches `regex`.
pub fn is_inst_call_func(inst: InstructionRef, regex: &regex::Regex) -> bool {
    inst.called_fn_name()
        .map(|n| regex.is_match(n))
        .unwrap_or(false)
}
