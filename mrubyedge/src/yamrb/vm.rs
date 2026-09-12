use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{array, env};

use crate::Error;
use crate::rite::{Irep, Rite, insn};

use super::op::Op;
use super::prelude::prelude;

// IREP ids must be unique across every loaded script: the VM keys closure
// environments by the enclosing irep's id (cur_env, has_env_ref, the
// __irep_id equality in op_return). Per-file numbering collides, letting a
// returning method from one file capture its registers into another file's
// live block environment.
static NEXT_IREP_ID: AtomicUsize = AtomicUsize::new(1);
use super::value::RHashMap;
use super::value::*;
use super::{op, optable::*};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ENGINE: &str = "mruby/edge";

/// FNV-1a 64 of a byte slice, matching `FnvBuildHasher` over the same bytes.
/// Used for dispatch-cache keys; independent of the ivar-map hasher feature.
pub(crate) fn fnv_hash(name: &str) -> u64 {
    use std::hash::Hasher;
    let mut h = fnv::FnvHasher::default();
    h.write(name.as_bytes());
    h.finish()
}

/// Cap for the inline argument buffer used at native boundaries. Small enough
/// to live on the stack; larger arities fall back to a caller-owned Vec.
pub(crate) const NATIVE_ARG_BUF: usize = 32;

/// Fresh `Option<Value>` argument buffer (values are not `Copy`, so `[None; N]`
/// repeat syntax is unavailable).
pub(crate) fn arg_buf() -> [Option<Value>; NATIVE_ARG_BUF] {
    std::array::from_fn(|_| None)
}

/// Wraps an unboxed `&[Value]` argument slice as `Option<Value>` (always
/// `Some`), using the stack buffer when it fits and reusing `out` otherwise.
pub(crate) fn value_args<'a>(
    args: &[Value],
    buf: &'a mut [Option<Value>; NATIVE_ARG_BUF],
    out: &'a mut Vec<Option<Value>>,
) -> &'a [Option<Value>] {
    if args.len() <= buf.len() {
        for (i, a) in args.iter().enumerate() {
            buf[i] = Some(a.clone());
        }
        &buf[..args.len()]
    } else {
        out.clear();
        out.extend(args.iter().cloned().map(Some));
        out.as_slice()
    }
}

/// Copies a register window into an unboxed `Option<Value>` argument slice,
/// failing loudly when a slot was never assigned (an internal invariant: the
/// compiler always initializes argument registers before a send).
pub(crate) fn reg_args<'a>(
    vm: &mut VM,
    start: usize,
    count: usize,
    buf: &'a mut [Option<Value>; NATIVE_ARG_BUF],
    out: &'a mut Vec<Option<Value>>,
) -> Result<&'a [Option<Value>], Error> {
    let regs = vm.current_regs();
    let fill = |i: usize| -> Result<Option<Value>, Error> {
        Ok(Some(regs[start + i].clone().ok_or_else(|| {
            Error::internal(format!("register {} is not assigned", start + i))
        })?))
    };
    if count <= buf.len() {
        for (i, slot) in buf[..count].iter_mut().enumerate() {
            *slot = fill(i)?;
        }
        Ok(&buf[..count])
    } else {
        out.clear();
        for i in 0..count {
            out.push(fill(i)?);
        }
        Ok(out.as_slice())
    }
}

pub(crate) const MAX_REGS_SIZE: usize = 256;

#[derive(Debug, Clone)]
pub enum TargetContext {
    Class(Rc<RClass>),
    Module(Rc<RModule>),
}

impl TargetContext {
    pub fn name(&self) -> String {
        match self {
            TargetContext::Class(c) => c.full_name(),
            TargetContext::Module(m) => m.full_name(),
        }
    }
}

/// Receiver identity for a lazy call-frame label. Kept as Rc clones (no
/// allocation); the class/module name is only formatted when the error
/// stack is captured.
#[derive(Clone)]
pub enum CallerReceiver {
    Class(Rc<RClass>),
    Module(Rc<RModule>),
    Instance(Rc<RClass>),
}

impl std::fmt::Debug for CallerReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Class(_) => write!(f, "CallerReceiver::Class"),
            Self::Module(_) => write!(f, "CallerReceiver::Module"),
            Self::Instance(_) => write!(f, "CallerReceiver::Instance"),
        }
    }
}

/// Lazy call-frame label. Built per send without allocating; the backtrace
/// formatter turns it into "ClassName#method" only when reporting errors.
#[derive(Clone)]
pub enum CallerLabel {
    /// Fixed label, no receiver qualification ("<tailcall>", "<exec>").
    Static(&'static str),
    /// Owned label, no receiver qualification ("super(method)").
    Owned(String),
    /// Rust-known method name on a receiver (mrb_funcall).
    Named {
        receiver: CallerReceiver,
        method: String,
    },
    /// Method resolved through a send: the name comes from the frame irep's
    /// sym table (or "method_missing" when the call went through a Ruby
    /// method_missing), so no per-send string allocation is needed.
    Send {
        receiver: CallerReceiver,
        sym_index: usize,
        use_method_missing: bool,
    },
}

impl std::fmt::Debug for CallerLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static(s) => write!(f, "Static({})", s),
            Self::Owned(s) => write!(f, "Owned({})", s),
            Self::Named { receiver, method } => {
                write!(f, "Named({:?}, {})", receiver, method)
            }
            Self::Send {
                sym_index,
                use_method_missing,
                ..
            } => write!(
                f,
                "Send(sym={}, method_missing={})",
                sym_index, use_method_missing
            ),
        }
    }
}

fn qualify(receiver: &CallerReceiver, method: &str) -> String {
    let class = match receiver {
        CallerReceiver::Class(c) => c.full_name(),
        CallerReceiver::Module(m) => m.sym_id.name.clone(),
        CallerReceiver::Instance(c) => c.full_name(),
    };
    format!("{class}#{method}")
}

/// Formats a call-frame label for backtraces. Send method names are resolved
/// through the crumb's frame irep here, at error time.
fn caller_label(label: &CallerLabel, irep: Option<&Rc<IREP>>) -> String {
    match label {
        CallerLabel::Static(s) => (*s).to_string(),
        CallerLabel::Owned(s) => s.clone(),
        CallerLabel::Named { receiver, method } => qualify(receiver, method.as_str()),
        CallerLabel::Send {
            receiver,
            sym_index,
            use_method_missing,
        } => {
            let method = if *use_method_missing {
                "method_missing"
            } else {
                irep.and_then(|i| i.syms.get(*sym_index))
                    .map(|s| s.name.as_str())
                    .unwrap_or("?")
            };
            qualify(receiver, method)
        }
    }
}

#[derive(Debug)]
pub struct Breadcrumb {
    pub event: &'static str, // TODO: be enum
    pub caller: Option<CallerLabel>,
    pub return_reg: Option<usize>,
    // caller's irep and pc at push time, for mapping a stack
    // frame to its source line via the irep's debug info.
    pub irep: Option<Rc<IREP>>,
    pub pc: Option<usize>,
    /// Monotonic id, unique per crumb, so a stale break anchor (whose frame
    /// was already popped) can be detected even without pointer identity.
    pub id: u64,
}

#[derive(Debug)]
pub struct KArgs {
    pub args: RefCell<RHashMap<RSym, Rc<RObject>>>,
    pub kwrest_reg: Cell<usize>,
    pub upper: Option<Rc<KArgs>>,
}

impl Breadcrumb {
    #[cfg(feature = "mrubyedge-debug")]
    pub fn display_breadcrumb_for_debug(&self) {
        eprintln!(
            "- Breadcrumb: event='{}', caller={}",
            self.event,
            self.caller
                .as_ref()
                .map(|c| caller_label(c, self.irep.as_ref()))
                .unwrap_or_else(|| "(none)".to_string()),
        );
    }
}

/// Name-keyed dispatch cache for `mrb_funcall`. Key: (method version, class
/// identity, fnv of the method name).
type MethodNameCache = HashMap<(u64, usize, u64), (Rc<RModule>, RProc)>;

pub struct VM {
    pub irep: Rc<IREP>,

    pub id: usize,
    pub bytecode: Vec<u8>,
    pub current_irep: Rc<IREP>,
    pub pc: Cell<usize>,
    pub regs: [Option<Value>; MAX_REGS_SIZE],
    pub current_regs_offset: usize,
    pub current_callinfo: Option<Rc<CALLINFO>>,
    // n_args of the running frame; call_block hides the
    // callinfo, and op_enter needs the count for optional arguments.
    pub current_n_args: Cell<usize>,
    // live call-frame stack (innermost last). One crumb per
    // call, pushed and popped like a stack; a Vec so steady-state pushes and
    // pops never allocate after the max call depth is reached.
    pub breadcrumbs: RefCell<Vec<Breadcrumb>>,
    // Monotonic crumb id source; ids are unique so stale break anchors can be
    // detected after their frame was popped.
    pub crumb_seq: Cell<u64>,
    // call stack of the last raised exception, captured at
    // raise time from the breadcrumb stack before unwinding destroys it.
    // Outermost frame first; only frames with a name are kept.
    pub last_error_stack: RefCell<Vec<String>>,
    // landing pad captured at OP_BREAK time — the nearest
    // do_op_send crumb's id and its return register. The unwinder delivers
    // the break value once that crumb is gone from the live stack.
    pub break_landing: RefCell<Option<(u64, usize)>>,
    // irep id of the script currently being evaluated. A block
    // return targeting it means there is no enclosing method (LocalJumpError).
    pub root_irep_id: Cell<Option<usize>>,
    pub kargs: RefCell<Option<RHashMap<RSym, Rc<RObject>>>>,
    pub current_kargs: RefCell<Option<Rc<KArgs>>>,
    pub target_class: TargetContext,
    pub exception: Option<Rc<RException>>,

    pub flag_preemption: Cell<bool>,

    #[cfg(feature = "insn-limit")]
    pub insn_count: Cell<usize>,
    #[cfg(feature = "insn-limit")]
    pub insn_limit: usize,

    // common class
    pub object_class: Rc<RClass>,
    pub builtin_class_table: RHashMap<&'static str, Rc<RClass>>,
    // hot-path class cache for RObject::get_class, avoiding a
    // builtin_class_table lookup per send. Filled from the prelude classes.
    pub class_class: Rc<RClass>,
    pub module_class: Rc<RClass>,
    pub integer_class: Rc<RClass>,
    pub float_class: Rc<RClass>,
    pub string_class: Rc<RClass>,
    pub array_class: Rc<RClass>,
    pub hash_class: Rc<RClass>,
    pub symbol_class: Rc<RClass>,
    pub proc_class: Rc<RClass>,
    pub range_class: Rc<RClass>,
    pub true_class: Rc<RClass>,
    pub false_class: Rc<RClass>,
    pub nil_class: Rc<RClass>,
    pub shared_memory_class: Rc<RClass>,
    pub class_object_table: RHashMap<String, Rc<RObject>>,

    pub globals: RHashMap<String, Value>,
    pub consts: RHashMap<String, Rc<RObject>>,

    pub upper: Option<Rc<ENV>>,
    // TODO: using fixed array?
    pub cur_env: RHashMap<usize, Rc<ENV>>,
    pub has_env_ref: RHashMap<usize, bool>,

    pub fn_table: RFnTable,
    pub fn_block_stack: RFnStack,

    /// Global method-definition version. Every definition, alias, undef or
    /// include bumps it, so dispatch caches stamp entries with it and go cold
    /// when it moves. Wrapping after 2^64 bumps is accepted as unreachable.
    pub method_version: Cell<u64>,
    /// Name-keyed dispatch cache for `mrb_funcall` (no bytecode call site).
    pub method_name_cache: RefCell<MethodNameCache>,
    /// `func` index of the pristine Array#[] native, captured after the
    /// prelude. A redefined Array#[] resolves to a different proc, which
    /// disables the GETIDX/SETIDX fast path.
    pub array_index_func: Cell<Option<usize>>,
    /// Identity registry for send fast paths: func index -> inline operation
    /// (numeric math or attr_accessor access). Populated at method registration
    /// through the `_fast` helpers; `do_op_send` consults it on a
    /// dispatch-cache hit only, so a redefined method (new func, bumped
    /// version) can never reuse a tag.
    pub fast_ops: RefCell<std::collections::HashMap<usize, FastOp>>,
    /// Ivar identity for attr_accessor fast-path closures, keyed by func index:
    /// the `@name` key shared with the closure and its precomputed FNV hash.
    pub fast_attrs: RefCell<std::collections::HashMap<usize, u32>>,
    /// Count of inline numeric/attr fast-path handlings (results and raised
    /// errors). Tests assert it moves to prove the fast path actually runs,
    /// and stays still after a redefinition replaced the tagged method.
    pub fast_native_hits: Cell<u64>,
    /// Count of attribute accesses served by `op_send`'s inline cache, which
    /// bypasses `do_op_send` entirely. Tests assert it moves so the op_send
    /// cache is observable independently of the do_op_send fast path.
    pub attr_cache_hits: Cell<u64>,
    /// Cached "Array index fast path is safe" verdict, reset on version bump.
    pub array_fast: Cell<Option<bool>>,
}

pub struct RFnTable {
    pub size: Cell<usize>,
    pub table: [MaybeUninit<Rc<RFn>>; 4096],
}

impl RFnTable {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            size: Cell::new(0),
            table: array::from_fn(|_| MaybeUninit::uninit()),
        }
    }

    pub fn set(&mut self, f: Rc<RFn>) {
        let i = self.size.get();
        if i >= self.table.len() {
            panic!("RFnTable overflow");
        }

        self.table[i].write(f);
        let size = self.size.get();
        if i >= size {
            self.size.set(i + 1);
        }
    }

    pub fn get(&self, i: usize) -> Option<Rc<RFn>> {
        if i >= self.size.get() {
            return None;
        }

        unsafe { self.table[i].assume_init_ref() }.clone().into()
    }

    pub fn len(&self) -> usize {
        self.size.get()
    }

    pub fn is_empty(&self) -> bool {
        self.size.get() == 0
    }
}

pub struct RFnStack {
    pub size: Cell<usize>,
    pub stack: [Option<Rc<RFn>>; 64],
}

impl RFnStack {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            size: Cell::new(0),
            stack: array::from_fn(|_| None),
        }
    }

    pub fn push(&mut self, f: Rc<RFn>) -> Result<(), Error> {
        let i = self.size.get();
        if i >= self.stack.len() {
            return Err(Error::internal("RFnStack overflow"));
        }

        self.stack[i] = Some(f);
        let size = self.size.get();
        if i >= size {
            self.size.set(i + 1);
        }
        Ok(())
    }

    pub fn pop(&self) -> Result<Rc<RFn>, Error> {
        let i = self.size.get();
        if i == 0 {
            return Err(Error::internal("RFnStack underflow"));
        }

        self.size.set(i - 1);

        self.stack[i - 1]
            .as_ref()
            .cloned()
            .ok_or_else(|| Error::internal("RFnStack invalid state"))
    }

    pub fn len(&self) -> usize {
        self.size.get()
    }

    pub fn is_empty(&self) -> bool {
        self.size.get() == 0
    }
}

// Break unwinding anchors on a do_op_send breadcrumb; a
// landing pad keeps that crumb's id, and this reports whether it is still
// alive in the live stack (popped frames are gone from the Vec).
fn breadcrumb_stack_contains(stack: &[Breadcrumb], target_id: u64) -> bool {
    stack.iter().any(|b| b.id == target_id)
}

impl VM {
    /// Builds a VM from a parsed Rite chunk, consuming the bytecode and
    /// preparing the VM so it can be executed via [`VM::run`].
    pub fn open(rite: &mut Rite) -> VM {
        let irep = rite_to_irep(rite);

        VM::new_by_raw_irep(irep)
    }

    /// Returns a VM backed by an empty IREP that immediately executes a
    /// `STOP` instruction. Useful for tests or placeholder VMs.
    pub fn empty() -> VM {
        let irep = IREP {
            __id: 0,
            nlocals: 0,
            nregs: 0,
            rlen: 0,
            code: vec![op::Op {
                code: insn::OpCode::STOP,
                operand: insn::Fetched::Z,
                pos: 18,
                len: 1,
            }],
            syms: Vec::new(),
            pool: Vec::new(),
            reps: Vec::new(),
            lv: None,
            catch_target_pos: Vec::new(),
            lines: Vec::new(),
            send_cache: RefCell::new(vec![None; 1]),
            attr_cache: RefCell::new(vec![None; 1]),
        };
        Self::new_by_raw_irep(irep)
    }

    /// Creates a VM directly from a raw [`IREP`] tree without going through the
    /// Rite loader. This wires up the register file, globals, and builtin
    /// tables and runs the prelude to seed standard classes.
    pub fn new_by_raw_irep(irep: IREP) -> VM {
        let irep = Rc::new(irep);
        let globals = RHashMap::default();
        let consts = RHashMap::default();
        let builtin_class_table = RHashMap::default();
        let class_object_table = RHashMap::default();

        let object_class = Rc::new(RClass::new("Object", None, None));
        object_class.update_module_weakref();

        let id = 1; // TODO generator
        let bytecode = Vec::new();
        let current_irep = irep.clone();
        let pc = Cell::new(0);
        let regs: [Option<Value>; MAX_REGS_SIZE] = [const { None }; MAX_REGS_SIZE];
        let current_regs_offset = 0;
        let current_callinfo = None;
        let current_n_args = Cell::new(0);
        let last_error_stack = RefCell::new(Vec::new());
        let break_landing = RefCell::new(None);
        let root_irep_id = Cell::new(None);
        let breadcrumbs = RefCell::new(Vec::new());
        let crumb_seq = Cell::new(0);
        let kargs = RefCell::new(None);
        let current_kargs = RefCell::new(None);
        let target_class = TargetContext::Class(object_class.clone());
        let exception = None;
        let flag_preemption = Cell::new(false);
        let fn_table = RFnTable::new();
        let fn_block_stack = RFnStack::new();
        let upper = None;
        let cur_env = RHashMap::default();
        let has_env_ref = RHashMap::default();

        #[cfg(feature = "insn-limit")]
        let insn_count = Cell::new(0);
        #[cfg(feature = "insn-limit")]
        let insn_limit = {
            let limit_str = env!(
                "MRUBYEDGE_INSN_LIMIT",
                "MRUBYEDGE_INSN_LIMIT must be set when insn-limit feature is enabled"
            );
            limit_str
                .parse::<usize>()
                .expect("MRUBYEDGE_INSN_LIMIT must be a valid number")
        };

        let mut vm = VM {
            id,
            bytecode,
            irep,
            current_irep,
            pc,
            regs,
            current_regs_offset,
            current_callinfo,
            current_n_args,
            last_error_stack,
            break_landing,
            root_irep_id,
            breadcrumbs,
            crumb_seq,
            kargs,
            current_kargs,
            target_class,
            exception,
            flag_preemption,
            #[cfg(feature = "insn-limit")]
            insn_count,
            #[cfg(feature = "insn-limit")]
            insn_limit,
            builtin_class_table,
            class_object_table,
            method_version: Cell::new(0),
            method_name_cache: RefCell::new(HashMap::new()),
            array_index_func: Cell::new(None),
            array_fast: Cell::new(None),
            fast_ops: RefCell::new(HashMap::new()),
            fast_attrs: RefCell::new(HashMap::new()),
            fast_native_hits: Cell::new(0),
            attr_cache_hits: Cell::new(0),
            // Placeholders; filled from the prelude classes below.
            class_class: object_class.clone(),
            module_class: object_class.clone(),
            integer_class: object_class.clone(),
            float_class: object_class.clone(),
            string_class: object_class.clone(),
            array_class: object_class.clone(),
            hash_class: object_class.clone(),
            symbol_class: object_class.clone(),
            proc_class: object_class.clone(),
            range_class: object_class.clone(),
            true_class: object_class.clone(),
            false_class: object_class.clone(),
            nil_class: object_class.clone(),
            shared_memory_class: object_class.clone(),
            object_class,
            globals,
            consts,
            upper,
            cur_env,
            has_env_ref,
            fn_table,
            fn_block_stack,
        };

        prelude(&mut vm);

        vm.class_class = vm.get_class_by_name("Class");
        vm.module_class = vm.get_class_by_name("Module");
        vm.integer_class = vm.get_class_by_name("Integer");
        vm.float_class = vm.get_class_by_name("Float");
        vm.string_class = vm.get_class_by_name("String");
        vm.array_class = vm.get_class_by_name("Array");
        vm.hash_class = vm.get_class_by_name("Hash");
        vm.symbol_class = vm.get_class_by_name("Symbol");
        vm.proc_class = vm.get_class_by_name("Proc");
        vm.range_class = vm.get_class_by_name("Range");
        vm.true_class = vm.get_class_by_name("TrueClass");
        vm.false_class = vm.get_class_by_name("FalseClass");
        vm.nil_class = vm.get_class_by_name("NilClass");
        vm.shared_memory_class = vm.get_class_by_name("SharedMemory");

        vm.array_index_func
            .set(resolve_method(&vm.array_class, "[]").and_then(|(_, m)| m.func));

        vm
    }

    /// Invalidates every dispatch cache: any method definition, alias, undef
    /// or include changes what send sites may resolve to.
    pub fn bump_method_version(&self) {
        self.method_version
            .set(self.method_version.get().wrapping_add(1));
        self.method_name_cache.borrow_mut().clear();
        self.array_fast.set(None);
    }

    /// Resolves a method by (class, name) through the name-keyed cache used
    /// by `mrb_funcall`. Entries are keyed on the method version and class
    /// identity, so a stale hit can never outlive a redefinition.
    pub fn resolve_method_cached(
        &self,
        klass: &Rc<RClass>,
        name: &str,
    ) -> Option<(Rc<RModule>, RProc)> {
        let version = self.method_version.get();
        let key = (version, Rc::as_ptr(klass) as usize, fnv_hash(name));
        if let Some((owner, method)) = self.method_name_cache.borrow().get(&key) {
            return Some((owner.clone(), method.clone()));
        }
        let resolved = resolve_method(klass, name);
        if let Some((owner, method)) = &resolved {
            self.method_name_cache
                .borrow_mut()
                .insert(key, (owner.clone(), method.clone()));
        }
        resolved
    }

    /// Resets the instruction counter. Only available when the `insn-limit` feature is enabled.
    #[cfg(feature = "insn-limit")]
    pub fn reset_insn_count(&mut self) {
        self.insn_count.set(0);
    }

    /// Returns the current instruction count. Only available when the `insn-limit` feature is enabled.
    #[cfg(feature = "insn-limit")]
    pub fn get_insn_count(&self) -> usize {
        self.insn_count.get()
    }

    /// Executes the current IREP until completion, returning the value in
    /// register 0 or propagating any raised exception as an error. The
    /// top-level `self` is initialized automatically before evaluation.
    pub fn run(&mut self) -> Result<Value, Box<dyn std::error::Error>> {
        self.current_irep = self.irep.clone();
        self.pc.set(0);

        self.push_breadcrumb(
            "run",
            None,
            None,
            Some(self.current_irep.clone()),
            Some(self.pc.get()),
        );
        self.__run()
    }

    /// Pushes a call-frame crumb with a fresh monotonic id. The Vec holds the
    /// crumbs by value, so steady-state pushes/pops never allocate.
    pub fn push_breadcrumb(
        &mut self,
        event: &'static str,
        caller: Option<CallerLabel>,
        return_reg: Option<usize>,
        irep: Option<Rc<IREP>>,
        pc: Option<usize>,
    ) {
        let id = self.crumb_seq.get().wrapping_add(1);
        self.crumb_seq.set(id);
        self.breadcrumbs.borrow_mut().push(Breadcrumb {
            event,
            caller,
            return_reg,
            irep,
            pc,
            id,
        });
    }

    /// Pops the innermost call-frame crumb.
    pub fn pop_breadcrumb(&mut self) {
        debug_assert!(
            !self.breadcrumbs.borrow().is_empty(),
            "crumb stack underflow"
        );
        self.breadcrumbs.borrow_mut().pop();
    }

    /// Internal run method that manages breadcrumb stack for internal calls.
    pub fn run_internal(&mut self) -> Result<Value, Box<dyn std::error::Error>> {
        self.push_breadcrumb(
            "run_internal",
            None,
            None,
            Some(self.current_irep.clone()),
            Some(self.pc.get()),
        );
        self.__run()
    }

    pub fn eval_rite(&mut self, rite: &mut Rite) -> Result<Value, Box<dyn std::error::Error>> {
        let irep = rite_to_irep(rite);
        self.pc.set(0);
        self.current_irep = Rc::new(irep);
        // the evaluated script is the outermost frame; a block
        // return unwinding to it has no enclosing method (LocalJumpError).
        self.root_irep_id.set(Some(self.current_irep.__id));

        // Each script evaluates against a fresh top-level self. A leftover
        // Class/Module in regs[0] from the previous file would otherwise make
        // top-level constant assignments (op_setconst) land in that stale
        // namespace instead of the global table.
        self.current_regs()[0] = None;

        self.push_breadcrumb(
            "eval",
            None,
            None,
            Some(self.current_irep.clone()),
            Some(self.pc.get()),
        );
        self.__run()
    }

    /// Source line of the opcode being (or just) executed, if the current
    /// irep carries debug info. The dispatch loop advances `pc` before an
    /// opcode runs, so the failing opcode sits at `pc - 1`.
    pub fn current_frame_line(&self) -> Option<u32> {
        self.current_irep
            .line_at_op(self.pc.get().saturating_sub(1))
    }

    /// call stack of the current exception, MRI-style. Each frame
    /// is a method label and the line where that method's body called the next
    /// frame (the inner crumb's recorded call site). The innermost frame is the
    /// failing callee and drops its line when the caller frame already shows it
    /// (the usual native-send/method_missing case). A synthetic <main> frame
    /// tops the stack at the top-level call site.
    pub fn capture_error_stack(&self) -> Vec<String> {
        // Innermost crumb first (the Vec grows outward); crumbs without a
        // caller label (top-level run/eval frames) are skipped.
        let crumbs: Vec<(String, Option<u32>)> = self
            .breadcrumbs
            .borrow()
            .iter()
            .rev()
            .filter_map(|b| {
                b.caller.as_ref().map(|label| {
                    let line = b
                        .irep
                        .as_ref()
                        .and_then(|i| b.pc.and_then(|p| i.line_at_op(p)));
                    (caller_label(label, b.irep.as_ref()), line)
                })
            })
            .collect();

        let mut frames: Vec<String> = Vec::new();
        let n = crumbs.len();
        if n == 0 {
            if let Some(line) = self.current_frame_line() {
                frames.push(format!("<main>:{line}"));
            }
            return frames;
        }

        let current_line = self.current_frame_line();
        // The failing line belongs to the caller frame when both resolve to the
        // same line; the callee frame then carries no line of its own.
        let innermost_line = if n >= 2 && current_line.is_some() && current_line == crumbs[0].1 {
            None
        } else {
            current_line
        };
        frames.push(match innermost_line {
            Some(l) => format!("{}:{l}", crumbs[0].0),
            None => crumbs[0].0.clone(),
        });
        for i in 1..n {
            let caller = &crumbs[i].0;
            frames.push(match crumbs[i - 1].1 {
                Some(l) => format!("{caller}:{l}"),
                None => caller.clone(),
            });
        }
        frames.push(match crumbs[n - 1].1 {
            Some(l) => format!("<main>:{l}"),
            None => "<main>".to_string(),
        });
        frames.reverse();
        frames
    }

    /// rejects a frame whose register window would overflow
    /// the fixed register array with a Ruby SystemStackError instead of a
    /// Rust panic (e.g. unbounded method_missing recursion).
    pub(crate) fn check_frame_window(&self, extra: usize, nregs: usize) -> Result<(), Error> {
        if self.current_regs_offset + extra + nregs > MAX_REGS_SIZE {
            return Err(Error::TaggedError(
                "SystemStackError".to_string(),
                "stack level too deep".to_string(),
            ));
        }
        Ok(())
    }

    fn __run(&mut self) -> Result<Value, Box<dyn std::error::Error>> {
        let class = self.object_class.clone();
        // Insert top_self
        let top_self = RObject {
            tt: RType::Instance,
            value: RValue::Instance(RInstance {
                class,
                ref_count: 1,
            }),
            object_id: 0.into(),
            singleton_class: RefCell::new(None),
            ivar: RefCell::new(IvarMap::new()),
        }
        .to_refcount_assigned();
        if self.current_regs()[0].is_none() {
            self.set_reg(0, top_self.clone());
        }
        let mut rescued = false;

        loop {
            if !rescued && let Some(e) = self.exception.clone() {
                let operand = insn::Fetched::B(0);
                // Break lands on the send recorded by OP_BREAK
                // (see break_landing). The unwinder pops frames until that
                // send's breadcrumb is gone, then delivers the value there.
                // Without a landing pad it unwinds like any other error.
                let break_landing: Option<(u64, usize)> =
                    if matches!(e.error_type.borrow().clone(), Error::Break(_)) {
                        *self.break_landing.borrow()
                    } else {
                        None
                    };
                if let Some(pos) = self.find_next_handler_pos() {
                    self.pc.set(pos);
                    rescued = true;
                    continue;
                }

                if let Error::BlockReturn(id, v) = e.error_type.borrow().clone()
                    && self.current_irep.__id == id
                {
                    // reached caller method's IREP, just return
                    let operand = insn::Fetched::B(16); // FIXME: just a bit far reg
                    self.set_reg(16, v);
                    self.exception.take();
                    op_return(self, &operand).expect("[bug]cannot return");
                    continue;
                }

                match op_return(self, &operand) {
                    Ok(_) => {
                        // once the anchored send crumb has been
                        // popped its frame is gone; deliver the break value
                        // into its return register and resume there.
                        if let Some((target_id, treg)) = &break_landing
                            && !breadcrumb_stack_contains(&self.breadcrumbs.borrow(), *target_id)
                            && let Error::Break(brkval) = e.error_type.borrow().clone()
                        {
                            self.set_reg(*treg, brkval);
                            self.exception.take();
                            self.break_landing.take();
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
                if self.flag_preemption.get() {
                    break;
                } else {
                    continue;
                }
            }
            rescued = false;

            let pc = self.pc.get();
            if pc >= self.current_irep.code.len() {
                // reached end of the IREP
                break;
            }
            let op = self.current_irep.code[pc];
            let operand = op.operand;
            self.pc.set(pc + 1);

            #[cfg(feature = "insn-limit")]
            {
                let count = self.insn_count.get();
                if count >= self.insn_limit {
                    return Err(Error::internal(format!(
                        "instruction limit exceeded: {} instructions",
                        self.insn_limit
                    ))
                    .into());
                }
                self.insn_count.set(count + 1);
            }

            #[cfg(feature = "mrubyedge-debug")]
            if let Ok(v) = env::var("MRUBYEDGE_DEBUG") {
                let level: i32 = v.parse().unwrap_or(1);
                if level >= 2 {
                    self.debug_dump_to_stdout(32);
                }
                eprintln!(
                    "{:?}: {:?} (pos={} len={})",
                    op.code, operand, op.pos, op.len
                );
            }

            match consume_expr(self, op.code, &operand, op.pos, op.len) {
                Ok(_) => {}
                Err(e) => {
                    // snapshot named breadcrumb frames at the
                    // deepest raise; skip while unwinding a pending
                    // exception, whose later re-conversions see popped
                    // chains. Last fresh raise wins.
                    if self.exception.is_none() {
                        *self.last_error_stack.borrow_mut() = self.capture_error_stack();
                    }
                    let exception = RException::from_error(self, &e);
                    self.exception = Some(Rc::new(exception));
                    continue;
                }
            }

            if self.flag_preemption.get() {
                break;
            }
        }

        self.flag_preemption.set(false);

        if let Some(e) = self.exception.clone() {
            return Err(e.error_type.borrow().clone().into());
        }

        let retval = match self.current_regs()[0].take() {
            Some(v) => Ok(v),
            None => Ok(Value::Nil),
        };
        self.set_reg(0, top_self.clone());

        retval
    }

    pub(crate) fn find_next_handler_pos(&mut self) -> Option<usize> {
        let ci = self.pc.get();
        for p in self.current_irep.catch_target_pos.iter() {
            if ci < *p {
                return Some(*p);
            }
        }
        None
    }

    pub(crate) fn current_regs(&mut self) -> &mut [Option<Value>] {
        &mut self.regs[self.current_regs_offset..]
    }

    /// Register read as a heap `RObject`, boxing an unboxed immediate. Native
    /// method boundaries use this; hot opcodes read [`Self::current_regs`]
    /// directly as [`Value`] to stay allocation-free.
    pub(crate) fn get_current_regs_cloned(&mut self, i: usize) -> Result<Rc<RObject>, Error> {
        self.current_regs()[i]
            .clone()
            .map(|v| v.to_rc())
            .ok_or_else(|| Error::internal(format!("register {} is not assigned", i)))
    }

    pub(crate) fn take_current_regs(&mut self, i: usize) -> Result<Rc<RObject>, Error> {
        self.current_regs()[i]
            .take()
            .map(|v| v.to_rc())
            .ok_or_else(|| Error::internal(format!("register {} is not assigned", i)))
    }

    /// Stores a heap result into a register, unboxing immediates so they never
    /// stay boxed, and returns the previous value boxed (the counterpart of
    /// [`Self::get_current_regs_cloned`]).
    pub(crate) fn set_reg(&mut self, i: usize, rc: Rc<RObject>) -> Option<Rc<RObject>> {
        self.current_regs()[i]
            .replace(Value::from_rc(rc))
            .map(|v| v.to_rc())
    }

    /// Register read as an unboxed `Value` (`Nil` when unassigned). Used by
    /// container opcodes so immediates never cross into `Rc<RObject>`.
    pub(crate) fn get_reg_value(&mut self, i: usize) -> Value {
        self.current_regs()[i].clone().unwrap_or(Value::Nil)
    }

    /// Register take as an unboxed `Value`, clearing the slot.
    pub(crate) fn take_reg_value(&mut self, i: usize) -> Value {
        self.current_regs()[i].take().unwrap_or(Value::Nil)
    }

    /// Store an unboxed `Value` into a register.
    pub(crate) fn set_reg_value(&mut self, i: usize, v: Value) {
        self.current_regs()[i] = Some(v);
    }

    /// Store an unboxed `Value` into a register, returning the previous slot.
    pub(crate) fn swap_reg_value(&mut self, i: usize, v: Value) -> Option<Value> {
        self.current_regs()[i].replace(v)
    }

    /// Returns the current `self` object from register 0, or an error if it has
    /// not been initialized yet.
    pub fn getself(&mut self) -> Result<Rc<RObject>, Error> {
        self.get_current_regs_cloned(0)
    }

    /// Retrieves `self` without error handling, panicking if register 0 is
    /// empty. Prefer [`VM::getself`] when the value may be absent.
    pub fn must_getself(&mut self) -> Rc<RObject> {
        self.current_regs()[0]
            .clone()
            .expect("self is not assigned")
            .to_rc()
    }

    pub fn get_kwargs(&self) -> Option<RHashMap<String, Rc<RObject>>> {
        let kwargs = self.current_kargs.borrow().clone();
        kwargs.map(|kargs| {
            kargs
                .args
                .borrow()
                .iter()
                .map(|(k, v)| (k.name.clone(), v.clone()))
                .collect()
        })
    }

    pub(crate) fn register_fn(&mut self, f: RFn) -> usize {
        self.fn_table.set(Rc::new(f));
        self.fn_table.len() - 1
    }

    /// Tags a registered native function so `do_op_send` can execute it inline
    /// (numeric math or attr_accessor access). The tag is looked up by `func`
    /// identity at a dispatch-cache hit, so it never outlives the exact
    /// registration.
    pub fn register_fast_native(&self, func: usize, op: FastOp) {
        self.fast_ops.borrow_mut().insert(func, op);
    }

    /// Records the ivar identity backing an attr_accessor fast-path closure;
    /// see [`Self::register_fast_native`] for the lifetime guarantee.
    pub fn register_fast_attr(&self, func: usize, key: u32) {
        self.fast_attrs.borrow_mut().insert(func, key);
    }

    pub(crate) fn push_fnblock(&mut self, f: Rc<RFn>) -> Result<(), Error> {
        self.fn_block_stack.push(f)
    }

    pub(crate) fn pop_fnblock(&mut self) -> Result<Rc<RFn>, Error> {
        self.fn_block_stack.pop()
    }

    pub(crate) fn get_fn(&self, i: usize) -> Option<Rc<RFn>> {
        self.fn_table.get(i)
    }

    /// Looks up a previously defined builtin class by name. Panics if the
    /// class does not exist, which usually signals a missing prelude setup.
    pub fn get_class_by_name(&self, name: &str) -> Rc<RClass> {
        self.builtin_class_table
            .get(name)
            .cloned()
            .unwrap_or_else(|| panic!("Class {} not found", name))
    }

    pub fn get_module_by_name(&self, name: &str) -> Rc<RModule> {
        match self.consts.get(name).cloned() {
            Some(obj) => match &obj.value {
                RValue::Module(m) => m.clone(),
                _ => panic!("Module {} not found", name),
            },
            None => panic!("Module {} not found", name),
        }
    }

    pub fn get_const_by_name(&self, name: &str) -> Option<Rc<RObject>> {
        self.consts.get(name).cloned()
    }

    /// Defines a new class under the optional parent module, inheriting from
    /// `superclass` or `Object` by default, and registers it in the constant
    /// table. The resulting class object is returned for further mutation.
    pub fn define_class(
        &mut self,
        name: &str,
        superclass: Option<Rc<RClass>>,
        parent_module: Option<Rc<RModule>>,
    ) -> Rc<RClass> {
        let superclass = match superclass {
            Some(c) => c,
            None => self.object_class.clone(),
        };
        let class = Rc::new(RClass::new(name, Some(superclass), parent_module.clone()));
        class.update_module_weakref();

        let object = RObject::class(class.clone(), self);
        self.consts.insert(name.to_string(), object.clone());
        if let Some(parent) = parent_module {
            parent
                .consts
                .borrow_mut()
                .insert(name.to_string(), object.clone());
        } else {
            self.object_class
                .consts
                .borrow_mut()
                .insert(name.to_string(), object);
        }
        class
    }

    /// Defines a new module, optionally nested under another module, and stores
    /// it in the VM's constant table so it becomes accessible to Ruby code.
    /// If a module with the same name already exists, it returns the existing one.
    pub fn define_module(&mut self, name: &str, parent_module: Option<Rc<RModule>>) -> Rc<RModule> {
        let existing = if let Some(ref parent) = parent_module {
            parent.consts.borrow().get(name).cloned()
        } else {
            self.consts.get(name).cloned()
        };
        if let Some(existing) = existing
            && let RValue::Module(ref m) = existing.value
        {
            return m.clone();
        }
        let module = Rc::new(RModule::new(name));
        if let Some(parent) = parent_module {
            module.parent.replace(Some(parent));
        }
        let object = RObject::module(module.clone()).to_refcount_assigned();
        self.consts.insert(name.to_string(), object.clone());
        self.object_class
            .consts
            .borrow_mut()
            .insert(name.to_string(), object);
        module
    }

    pub(crate) fn define_standard_class(&mut self, name: &'static str) -> Rc<RClass> {
        let class = self.define_class(name, None, None);
        self.builtin_class_table.insert(name, class.clone());
        class
    }

    pub(crate) fn define_standard_class_with_superclass(
        &mut self,
        name: &'static str,
        superclass: Rc<RClass>,
    ) -> Rc<RClass> {
        let class = self.define_class(name, Some(superclass.clone()), None);
        self.builtin_class_table.insert(name, class.clone());
        class
    }

    #[allow(dead_code)]
    pub(crate) fn define_standard_class_under(
        &mut self,
        name: &'static str,
        parent: Rc<RModule>,
    ) -> Rc<RClass> {
        let class = self.define_class(name, None, Some(parent));
        self.builtin_class_table.insert(name, class.clone());
        class
    }

    #[allow(dead_code)]
    pub(crate) fn define_standard_class_with_superclass_under(
        &mut self,
        name: &'static str,
        superclass: Rc<RClass>,
        parent: Rc<RModule>,
    ) -> Rc<RClass> {
        let class = self.define_class(name, Some(superclass.clone()), Some(parent));
        self.builtin_class_table.insert(name, class.clone());
        class
    }

    #[allow(unused)]
    pub fn debug_dump_to_stdout(&mut self, max_breadcrumb_level: usize) {
        #[cfg(feature = "mrubyedge-debug")]
        {
            use crate::yamrb::helpers::mrb_call_inspect;
            eprintln!("=== VM Dump ===");
            eprintln!("ID: {}", self.id);
            eprintln!("Current IREP ID: {}", self.current_irep.__id);
            eprintln!("PC: {}", self.pc.get());
            let current_regs_offset = self.current_regs_offset;
            eprintln!("IREPs:");
            self.current_irep
                .code
                .iter()
                .enumerate()
                .for_each(|(i, op)| {
                    eprintln!("{:04} {:?}: {:?}", i, op.code, op.operand);
                });
            eprintln!("Current Regs Offset: {}", current_regs_offset);
            eprintln!("Regs:");
            let size = self.regs.len();
            for i in 0..size {
                let reg = self.regs.get(i).unwrap().clone();
                if let Some(obj) = reg {
                    let rc = obj.to_rc();
                    let insp = mrb_call_inspect(self, rc.clone()).unwrap();
                    let inspect: String = (&insp)
                        .try_into()
                        .unwrap_or_else(|_| "(uninspectable)".into());
                    if i < current_regs_offset {
                        eprintln!("  R{}(--): {}(oid={})", i, inspect, rc.object_id.get());
                    } else {
                        eprintln!(
                            "  R{}(R{}): {}(oid={})",
                            i,
                            i - current_regs_offset,
                            inspect,
                            rc.object_id.get()
                        );
                    }
                } else if i < 16 || i < current_regs_offset {
                    eprintln!("  R{}(--): <None>", i);
                } else {
                    break;
                }
            }
            // eprintln!("Current CallInfo: {:?}", self.current_callinfo);
            eprintln!("Target Class: {}", self.target_class.name());
            eprintln!(
                "Exception: {:?}",
                self.exception
                    .as_deref()
                    .map(|e| e.error_type.borrow().clone())
            );
            eprintln!("--- Breadcrumb ---");
            let crumbs = self.breadcrumbs.borrow();
            if crumbs.is_empty() {
                eprintln!("(none)");
            }
            for bc in crumbs.iter().rev().take(max_breadcrumb_level) {
                bc.display_breadcrumb_for_debug();
            }
            eprintln!("=== End of VM Dump ===");
        }
    }

    pub fn get_outermost_env(&self) -> Option<Rc<ENV>> {
        let mut env = self.upper.clone();
        while let Some(e) = env.clone() {
            if e.upper.is_none() {
                return env;
            }
            env = e.upper.clone();
        }
        env
    }
}

fn interpret_insn(mut insns: &[u8]) -> Vec<Op> {
    let mut pos: usize = 0;
    let mut ops = Vec::new();
    while !insns.is_empty() {
        let op = insns[0];
        let opcode: insn::OpCode = op.try_into().unwrap();
        let fetched = insn::FETCH_TABLE[op as usize](&mut insns).unwrap();
        ops.push(Op::new(opcode, fetched, pos, 1 + fetched.len()));
        pos += 1 + fetched.len();
    }
    ops
}

fn load_irep_1(reps: &mut [Irep], pos: usize) -> (IREP, usize) {
    let irep = &mut reps[pos];
    let mut irep1 = IREP {
        __id: NEXT_IREP_ID.fetch_add(1, Ordering::SeqCst),
        nlocals: irep.nlocals(),
        nregs: irep.nregs(),
        rlen: irep.rlen(),
        code: Vec::new(),
        syms: Vec::new(),
        pool: Vec::new(),
        reps: Vec::new(),
        lv: None,
        catch_target_pos: Vec::new(),
        lines: irep.lines.clone(),
        send_cache: RefCell::new(Vec::new()),
        attr_cache: RefCell::new(Vec::new()),
    };
    for sym in irep.syms.iter() {
        irep1
            .syms
            .push(RSym::new(sym.to_string_lossy().to_string()));
    }
    for val in irep.pool.iter() {
        match val {
            crate::rite::PoolValue::Str(s) | crate::rite::PoolValue::SStr(s) => {
                irep1.pool.push(RPool::Str(s.to_string_lossy().to_string()));
            }
            crate::rite::PoolValue::Int32(i) => {
                irep1.pool.push(RPool::Int(*i as i64));
            }
            crate::rite::PoolValue::Int64(i) => {
                irep1.pool.push(RPool::Int(*i));
            }
            crate::rite::PoolValue::Float(f) => {
                irep1.pool.push(RPool::Float(*f));
            }
            crate::rite::PoolValue::BigInt(_) => {
                // BigInt not yet supported, store as 0 for now
                irep1.pool.push(RPool::Int(0));
            }
        }
    }
    let code = interpret_insn(irep.insn);
    for ch in irep.catch_handlers.iter() {
        let pos = ch.target;
        let (i, _) = code
            .iter()
            .enumerate()
            .find(|(_, op)| op.pos == pos)
            .expect("catch handler mismatch");
        irep1.catch_target_pos.push(i);
    }
    let mut map = RHashMap::default();
    for (reg, name) in irep.lv.iter().enumerate() {
        if let Some(name) = name {
            // lv register index in mruby is 1-based
            map.insert(reg + 1, name.to_string_lossy().to_string());
        }
    }
    if !map.is_empty() {
        irep1.lv = Some(map);
    }
    irep1.catch_target_pos.sort();

    irep1.code = code;
    irep1.send_cache = RefCell::new(vec![None; irep1.code.len()]);
    irep1.attr_cache = RefCell::new(vec![None; irep1.code.len()]);
    (irep1, pos + 1)
}

fn load_irep_0(reps: &mut [Irep], pos: usize) -> (IREP, usize) {
    let (mut irep0, newpos) = load_irep_1(reps, pos);
    let mut pos = newpos;
    for _ in 0..irep0.rlen {
        let (rep, newpos) = load_irep_0(reps, pos);
        pos = newpos;
        irep0.reps.push(Rc::new(rep));
    }
    (irep0, pos)
}

// This will consume the Rite object and return the IREP
fn rite_to_irep(rite: &mut Rite) -> IREP {
    let (irep0, _) = load_irep_0(&mut rite.irep, 0);
    irep0
}

#[derive(Debug, Clone)]
pub struct IREP {
    pub __id: usize,

    pub nlocals: usize,
    pub nregs: usize, // NOTE: is u8 better?
    pub rlen: usize,
    pub code: Vec<Op>,
    pub syms: Vec<RSym>,
    pub pool: Vec<RPool>,
    pub reps: Vec<Rc<IREP>>,
    pub lv: Option<RHashMap<usize, String>>,
    pub catch_target_pos: Vec<usize>,
    /// Source line changes (instruction-byte offset, line), ascending.
    /// Empty when the blob was compiled without debug info.
    pub lines: Vec<(u32, u32)>,
    /// Inline method dispatch cache, one slot per instruction. `do_op_send`
    /// reads and fills its slot at the instruction index; entries go stale
    /// when the global method version moves.
    pub send_cache: RefCell<Vec<Option<SendCacheEntry>>>,
    /// Attribute inline cache, one slot per instruction, parallel to
    /// `send_cache`. When a send site resolves to an attr_accessor getter or
    /// setter, its slot is filled so `op_send` can execute the access as a
    /// direct IvarMap read/write without entering `do_op_send`. Entries go
    /// stale with the method version, exactly like `send_cache`.
    pub attr_cache: RefCell<Vec<Option<AttrCacheEntry>>>,
}

/// One inline cache slot: the last method a bytecode send site resolved to
/// for a given receiver class, stamped with the method version at fill time.
#[derive(Debug, Clone)]
pub struct SendCacheEntry {
    pub version: u64,
    pub klass: Rc<RClass>,
    pub owner: Rc<RModule>,
    pub method: RProc,
}

/// Attribute inline-cache slot: the ivar an attr_accessor send site maps to
/// for a given receiver class, stamped with the method version at fill time.
/// A hit means the previous resolution was the pristine accessor, so the same
/// class and version cannot have redefined it.
#[derive(Debug, Clone)]
pub struct AttrCacheEntry {
    pub version: u64,
    pub klass: Rc<RClass>,
    pub key: u32,
    pub is_set: bool,
}

impl IREP {
    /// Source line of the opcode at `pc`, if the irep carries debug info.
    pub fn line_at_op(&self, pc: usize) -> Option<u32> {
        let op = self.code.get(pc)?;
        crate::rite::line_at(&self.lines, op.pos as u32)
    }
}

#[derive(Debug, Clone)]
pub struct CALLINFO {
    pub prev: Option<Rc<CALLINFO>>,
    pub method_id: RSym,
    pub pc_irep: Rc<IREP>,
    pub pc: usize,
    pub current_regs_offset: usize,
    pub target_class: TargetContext,
    pub n_args: usize,
    pub return_reg: usize,
    pub method_owner: Option<Rc<RModule>>,
    pub has_block: Cell<bool>,
    // whether op_enter pushed a KArgs frame for this call, so
    // op_return only pops one when it was actually pushed.
    pub kargs_pushed: Cell<bool>,
    // true for frames entered through call_block (funcalls
    // from native code, blocks). Their return preempts back to the native
    // caller instead of restoring a Ruby caller, but the callinfo stays live
    // while the callee runs so super/op_enter can read it.
    pub is_funcall: bool,
}

#[derive(Debug, Clone)]
pub struct ENV {
    pub __irep_id: usize,
    pub upper: Option<Rc<ENV>>,
    pub captured: RefCell<Option<Vec<Option<Rc<RObject>>>>>,
    pub current_regs_offset: usize,
    pub is_expired: Cell<bool>,
    // lambda closures return locally (CRuby semantics); the
    // flag and the closure's own irep id are set by op_lambda/op_block.
    pub is_lambda: Cell<bool>,
    pub closure_irep_id: usize,
}

impl ENV {
    #[allow(unused)]
    pub(crate) fn has_captured(&self) -> bool {
        self.captured.borrow().is_some()
    }

    #[allow(unused)]
    pub(crate) fn capture(&self, regs: &[Option<Rc<RObject>>]) {
        let mut captured = self.captured.borrow_mut();
        captured.replace(regs.to_vec());
    }

    pub(crate) fn capture_no_clone(&self, regs: Vec<Option<Rc<RObject>>>) {
        let mut captured = self.captured.borrow_mut();
        captured.replace(regs);
    }

    pub(crate) fn expire(&self) {
        self.is_expired.set(true);
    }

    pub(crate) fn expired(&self) -> bool {
        self.is_expired.get()
    }
}
