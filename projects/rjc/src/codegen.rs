//! Emits textual LLVM IR from the RJ IR.
//!
//! Calling convention (matching `stdlib/start.ll`): every function returns
//! `void` and takes one pointer per return value first, then its arguments.
//! A call passes the destination locals' storage as the return pointers.
//! Scalar arguments are passed by value. Array arguments depend on the
//! parameter's mode (see [`crate::ir::modes`]):
//!
//! - read: a pointer to an array the callee must not change. The caller lends
//!   its own array unless the call also writes that storage.
//! - consume: a pointer to a buffer the callee owns. The caller hands over its
//!   own array if it doesn't need the value afterwards, otherwise a copy.
//! - in/out: no separate argument. The parameter lives in its return slot's
//!   buffer, which the caller fills with the argument. For `x = f(x)` that is
//!   just `x`'s storage, so nothing is copied.
//!
//! Every other local gets a stack slot (`alloca`) in the entry block. LLVM's
//! `mem2reg` turns scalar slots into registers when optimising.
//!
//! Array indexing is bounds checked; an out-of-bounds index calls
//! `llvm.trap`, which kills the program.
//!
//! Functions are emitted as `rj_<name>` so they can't collide with symbols
//! from other objects (libc's `exit`, for example).

use std::{collections::HashSet, fmt::Write as _};

use crate::diagnostic::{Code, Error, Span};
use crate::ir::{
    liveness::{liveness, Liveness},
    modes::{param_modes, ParamMode},
    *,
};

pub fn emit(program: &Program, source_name: &str) -> String {
    let mut out = String::new();

    writeln!(out, "source_filename = \"{}\"", source_name.escape_default()).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)").unwrap();
    writeln!(out, "declare void @llvm.trap()").unwrap();

    if !program.structs.is_empty() {
        writeln!(out).unwrap();
    }
    for def in &program.structs {
        let fields = def.fields.iter().map(|f| llvm_type(&f.ty)).collect::<Vec<_>>();
        writeln!(out, "%struct.{} = type {{ {} }}", def.name, fields.join(", ")).unwrap();
    }

    let modes = program.functions.iter().map(param_modes).collect::<Vec<_>>();

    for (i, function) in program.functions.iter().enumerate() {
        writeln!(out).unwrap();
        out.push_str(&FunctionEmitter::new(program, function, &modes, &modes[i]).emit().0);
    }

    out
}

/// A note for every array or struct copy the generated code makes, saying
/// why the copy couldn't be avoided.
pub fn copy_notes(program: &Program) -> Vec<Error> {
    let modes = program.functions.iter().map(param_modes).collect::<Vec<_>>();

    program
        .functions
        .iter()
        .enumerate()
        .flat_map(|(i, function)| FunctionEmitter::new(program, function, &modes, &modes[i]).emit().1)
        .collect()
}

pub fn symbol(name: &str) -> String {
    format!("rj_{name}")
}

fn llvm_type(ty: &Type) -> String {
    match ty {
        Type::I32 => "i32".to_string(),
        Type::Bool => "i1".to_string(),
        Type::Array(elem, len) => format!("[{len} x {}]", llvm_type(elem)),
        Type::Struct(_, name) => format!("%struct.{name}"),
        Type::Named(..) => unreachable!("named types are erased by the checker"),
    }
}

/// Size in bytes as LLVM lays the type out, as a constant expression.
fn size_of(ty: &Type) -> String {
    format!(
        "ptrtoint (ptr getelementptr ({}, ptr null, i32 1) to i64)",
        llvm_type(ty)
    )
}

fn local_slot(id: LocalId) -> String {
    format!("%l{}", id.0)
}

struct FunctionEmitter<'a> {
    program: &'a Program,
    function: &'a Function,
    /// Parameter modes of every function, by `FuncId`.
    modes: &'a [Vec<ParamMode>],
    /// Modes of this function's parameters.
    own_modes: &'a [ParamMode],
    liveness: Liveness,
    /// Allocas, placed in the entry block so loops don't grow the stack.
    entry: String,
    body: String,
    /// Label of the LLVM block currently being written.
    label: String,
    next_register: usize,
    next_label: usize,
    uses_trap: bool,
    /// The source statement being emitted, and notes about copies in it.
    span: Span,
    notes: Vec<Error>,
}

impl<'a> FunctionEmitter<'a> {
    fn new(
        program: &'a Program,
        function: &'a Function,
        modes: &'a [Vec<ParamMode>],
        own_modes: &'a [ParamMode],
    ) -> Self {
        FunctionEmitter {
            program,
            function,
            modes,
            own_modes,
            liveness: liveness(function),
            entry: String::new(),
            body: String::new(),
            label: String::new(),
            next_register: 0,
            next_label: 0,
            uses_trap: false,
            span: Span::new(0, 0),
            notes: vec![],
        }
    }

    fn line(&mut self, line: impl AsRef<str>) {
        self.body.push_str("  ");
        self.body.push_str(line.as_ref());
        self.body.push('\n');
    }

    fn start_block(&mut self, label: String) {
        writeln!(self.body, "{label}:").unwrap();
        self.label = label;
    }

    fn register(&mut self) -> String {
        self.next_register += 1;
        format!("%r{}", self.next_register)
    }

    fn fresh_label(&mut self, prefix: &str) -> String {
        self.next_label += 1;
        format!("{prefix}{}", self.next_label)
    }

    fn ty(&self, id: LocalId) -> Type {
        self.function.local(id).ty.clone()
    }

    fn param_mode(&self, id: LocalId) -> Option<ParamMode> {
        let index = self.function.params.iter().position(|p| *p == id)?;
        Some(self.own_modes[index])
    }

    /// Name of the pointer to a local's storage. An in/out parameter lives in
    /// its return slot's buffer.
    fn slot(&self, id: LocalId) -> String {
        match self.param_mode(id) {
            Some(ParamMode::InOut(ret)) => local_slot(self.function.returns[ret]),
            _ => local_slot(id),
        }
    }

    /// Whether this function may let a callee overwrite `x`'s storage. Read
    /// parameters belong to our caller, and an in/out parameter's storage is
    /// our own return value.
    fn owns(&self, x: LocalId) -> bool {
        !matches!(self.param_mode(x), Some(ParamMode::Read | ParamMode::InOut(_)))
    }

    /// A stack buffer for a temporary copy, allocated in the entry block.
    fn temp_array(&mut self, ty: &Type) -> String {
        let name = format!("%copy{}", self.fresh_label(""));
        writeln!(self.entry, "  {name} = alloca {}", llvm_type(ty)).unwrap();
        name
    }

    /// Records why the copy just emitted was needed.
    fn note(&mut self, message: String, help: Option<&str>) {
        let mut note = Error::note(Code::Copy, message, self.span);
        if let Some(help) = help {
            note.help(help);
        }
        self.notes.push(note);
    }

    /// How a place reads in source: `g.nodes[i].edges`, `a temporary`.
    fn describe(&self, local: LocalId, path: &[Elem]) -> String {
        let local_info = self.function.local(local);
        let mut out = match &local_info.kind {
            LocalKind::Param(name) | LocalKind::Var(name) => format!("`{name}"),
            LocalKind::Return => return "the return value".to_string(),
            LocalKind::Temp if path.is_empty() => return "a temporary".to_string(),
            LocalKind::Temp => "`(...)".to_string(),
        };

        let mut ty = local_info.ty.clone();
        for elem in path {
            ty = match (elem, ty) {
                (Elem::Index(index), Type::Array(elem, _)) => {
                    let index = match index {
                        Operand::Int(i) => i.to_string(),
                        Operand::Local(id) => match &self.function.local(*id).kind {
                            LocalKind::Param(name) | LocalKind::Var(name) => name.clone(),
                            _ => "...".to_string(),
                        },
                        Operand::Bool(b) => b.to_string(),
                    };
                    out += &format!("[{index}]");
                    *elem
                }
                (Elem::Field(field), Type::Struct(id, _)) => {
                    let field = &self.program.structs[id.0].fields[*field];
                    out += &format!(".{}", field.name);
                    field.ty.clone()
                }
                (_, ty) => unreachable!("path does not match type {ty}"),
            };
        }

        out + "`"
    }

    fn emit(mut self) -> (String, Vec<Error>) {
        let function = self.function;

        let mut params = vec![];
        for ret in &function.returns {
            params.push(format!("ptr {}", local_slot(*ret)));
        }
        for (param, mode) in function.params.iter().zip(self.own_modes) {
            match mode {
                ParamMode::Value => {
                    params.push(format!("{} %p{}", llvm_type(&self.ty(*param)), param.0))
                }
                ParamMode::Read | ParamMode::Consume => {
                    params.push(format!("ptr {}", local_slot(*param)))
                }
                ParamMode::InOut(_) => {}
            }
        }

        for (i, local) in function.locals.iter().enumerate() {
            let id = LocalId(i);
            let is_aggregate_param = function.params.contains(&id) && local.ty.is_aggregate();

            if local.kind != LocalKind::Return && !is_aggregate_param {
                writeln!(self.entry, "  {} = alloca {}", local_slot(id), llvm_type(&local.ty))
                    .unwrap();
            }
        }

        for param in &function.params {
            let ty = self.ty(*param);

            if ty.is_scalar() {
                writeln!(
                    self.entry,
                    "  store {} %p{}, ptr {}",
                    llvm_type(&ty),
                    param.0,
                    local_slot(*param)
                )
                .unwrap();
            }
        }

        for (i, block) in function.blocks.iter().enumerate() {
            self.start_block(format!("bb{i}"));

            for (j, stmt) in block.stmts.iter().enumerate() {
                self.span = block.spans[j];
                self.stmt(stmt, i, j);
            }

            self.terminator(&block.terminator);
        }

        if self.uses_trap {
            self.start_block("trap".to_string());
            self.line("call void @llvm.trap()");
            self.line("unreachable");
        }

        let text = format!(
            "define void @{}({}) {{\nentry:\n{}  br label %bb0\n{}}}\n",
            symbol(&function.name),
            params.join(", "),
            self.entry,
            self.body
        );

        (text, self.notes)
    }

    fn operand_type(&self, operand: Operand) -> Type {
        match operand {
            Operand::Local(id) => self.ty(id),
            Operand::Int(_) => Type::I32,
            Operand::Bool(_) => Type::Bool,
        }
    }

    /// Returns the LLVM value of a scalar operand, loading locals.
    fn operand(&mut self, operand: Operand) -> String {
        match operand {
            Operand::Local(id) => {
                let ty = self.ty(id);
                debug_assert!(ty.is_scalar(), "aggregate used as a scalar operand");

                let register = self.register();
                let slot = self.slot(id);
                self.line(format!("{register} = load {}, ptr {slot}", llvm_type(&ty)));
                register
            }
            Operand::Int(value) => value.to_string(),
            Operand::Bool(value) => value.to_string(),
        }
    }

    fn memcpy(&mut self, dest: &str, src: &str, ty: &Type) {
        self.line(format!(
            "call void @llvm.memcpy.p0.p0.i64(ptr {dest}, ptr {src}, i64 {}, i1 false)",
            size_of(ty)
        ));
    }

    /// Bounds-checks `index` against `len` and continues in a new block.
    fn bounds_check(&mut self, index: &str, len: u32) {
        let in_bounds = self.register();
        let next = self.fresh_label("inbounds");

        // Unsigned comparison also rejects negative indices.
        self.line(format!("{in_bounds} = icmp ult i32 {index}, {len}"));
        self.line(format!("br i1 {in_bounds}, label %{next}, label %trap"));
        self.uses_trap = true;
        self.start_block(next);
    }

    /// A pointer to the part of `local` that `path` leads to, and its type.
    /// Indexes are bounds-checked.
    fn place_ptr(&mut self, local: LocalId, path: &[Elem]) -> (String, Type) {
        let mut ptr = self.slot(local);
        let mut ty = self.ty(local);

        for elem in path {
            let next = self.register();

            ty = match (elem, &ty) {
                (Elem::Index(index), Type::Array(elem_ty, len)) => {
                    let index = self.operand(*index);
                    self.bounds_check(&index, *len);
                    self.line(format!(
                        "{next} = getelementptr {}, ptr {ptr}, i64 0, i32 {index}",
                        llvm_type(&ty)
                    ));
                    (**elem_ty).clone()
                }
                (Elem::Field(field), Type::Struct(id, _)) => {
                    self.line(format!(
                        "{next} = getelementptr {}, ptr {ptr}, i32 0, i32 {field}",
                        llvm_type(&ty)
                    ));
                    self.program.structs[id.0].fields[*field].ty.clone()
                }
                _ => unreachable!("path does not match type {ty}"),
            };
            ptr = next;
        }

        (ptr, ty)
    }

    /// Stores `value` of type `ty` at `ptr`, described as `dest` in notes;
    /// aggregates are copied.
    fn store_at(&mut self, ptr: &str, value: Operand, ty: &Type, dest: &str) {
        if ty.is_aggregate() {
            let Operand::Local(src) = value else {
                unreachable!("aggregate operand is not a local");
            };
            // An in/out parameter returned as itself shares its slot.
            let from = self.slot(src);
            if from != ptr {
                self.memcpy(ptr, &from, ty);
                let src = self.describe(src, &[]);
                self.note(format!("copies {src} into {dest}"), None);
            }
        } else {
            let value = self.operand(value);
            self.line(format!("store {} {value}, ptr {ptr}", llvm_type(ty)));
        }
    }

    /// Copies the value of type `ty` at `from` to `to`, described as `what`
    /// in notes.
    fn copy_at(&mut self, to: &str, from: &str, ty: &Type, what: &str) {
        if ty.is_aggregate() {
            self.memcpy(to, from, ty);
            self.note(format!("copies {what}"), None);
        } else {
            let register = self.register();
            let llvm_ty = llvm_type(ty);
            self.line(format!("{register} = load {llvm_ty}, ptr {from}"));
            self.line(format!("store {llvm_ty} {register}, ptr {to}"));
        }
    }

    /// `block` and `index` locate the statement, for liveness.
    fn stmt(&mut self, stmt: &Stmt, block: usize, index: usize) {
        match stmt {
            Stmt::Assign { dest, value } => self.assign(*dest, value),

            Stmt::SetPlace { local, path, value } => {
                let (ptr, ty) = self.place_ptr(*local, path);
                let dest = self.describe(*local, path);
                self.store_at(&ptr, *value, &ty, &dest);
            }

            Stmt::Call { func, args, dests } => {
                let live_after = self.liveness.live_after[block][index].clone();
                self.call(*func, args, dests, &live_after);
            }
        }
    }

    /// The type of the part of `local` that `path` leads to.
    fn path_type(&self, local: LocalId, path: &[Elem]) -> Type {
        let mut ty = self.ty(local);

        for elem in path {
            ty = match (elem, ty) {
                (Elem::Index(_), Type::Array(elem, _)) => *elem,
                (Elem::Field(field), Type::Struct(id, _)) => {
                    self.program.structs[id.0].fields[*field].ty.clone()
                }
                (_, ty) => unreachable!("path does not match type {ty}"),
            };
        }

        ty
    }

    /// Emits a call. Array and struct arguments are passed according to the
    /// callee's parameter modes; see the module documentation. Two places
    /// overlap if they belong to the same variable ("root"), compared by
    /// storage so an in/out parameter and its return slot count as one.
    fn call(&mut self, func: FuncId, args: &[CallArg], dests: &[Place], live_after: &HashSet<LocalId>) {
        let all_modes = self.modes;
        let modes = &all_modes[func.0];

        let arg_places = args
            .iter()
            .map(|arg| match arg {
                CallArg::Place(place) => Some(place.clone()),
                CallArg::Value(Operand::Local(x)) if self.ty(*x).is_aggregate() => {
                    Some(Place::whole(*x))
                }
                CallArg::Value(_) => None,
            })
            .collect::<Vec<_>>();
        let read_roots = arg_places
            .iter()
            .flatten()
            .map(|place| self.slot(place.local))
            .collect::<Vec<_>>();
        let dest_roots = dests.iter().map(|dest| self.slot(dest.local)).collect::<Vec<_>>();
        let count = |roots: &[String], root: &str| roots.iter().filter(|r| *r == root).count();

        // Destination pointers; their indexes are checked before the call.
        let mut ret_ptrs = vec![];
        for dest in dests {
            ret_ptrs.push(self.place_ptr(dest.local, &dest.path).0);
        }
        let mut copy_back = vec![];

        // For an in/out parameter, the return slot's buffer must hold the
        // argument when the call starts.
        for (k, mode) in modes.iter().enumerate() {
            let ParamMode::InOut(ret) = *mode else {
                continue;
            };
            let arg = arg_places[k].clone().expect("in/out argument is not a place");
            let dest = &dests[ret];
            let root = self.slot(arg.local);
            let ty = self.path_type(arg.local, &arg.path);

            // `x = f(x)` or `g.nodes[i] = f(g.nodes[i])`: the place is
            // overwritten by the result, so it's updated where it is, as long
            // as nothing else in the call refers to the same variable.
            let in_place = arg == *dest
                && count(&read_roots, &root) == 1
                && count(&dest_roots, &root) == 1
                && self.param_mode(arg.local) != Some(ParamMode::Read);

            if in_place {
                continue;
            }

            let (from, _) = self.place_ptr(arg.local, &arg.path);

            let callee = &self.program.function(func).name;
            let (arg_desc, dest_desc) = (
                self.describe(arg.local, &arg.path),
                self.describe(dest.local, &dest.path),
            );

            if count(&read_roots, &dest_roots[ret]) == 0 {
                // Nothing else in the call reads the destination's variable,
                // so fill the destination first.
                let to = ret_ptrs[ret].clone();
                self.memcpy(&to, &from, &ty);
                self.note(
                    format!(
                        "copies {arg_desc} into {dest_desc} for `{callee}`, because the result goes to a \
                         different place than the argument"
                    ),
                    Some("assign the result back to the argument (`x = f(x)`) to update it in place"),
                );
            } else {
                let temp = self.temp_array(&ty);
                self.memcpy(&temp, &from, &ty);
                let root = self.describe(dest.local, &[]);
                self.note(
                    format!(
                        "copies {arg_desc} in and out of a temporary for `{callee}`, because the call \
                         also reads {root}, where the result goes"
                    ),
                    None,
                );
                let original = std::mem::replace(&mut ret_ptrs[ret], temp.clone());
                copy_back.push((temp, original, ty));
            }
        }

        // Variables the callee writes during the call.
        let written = dest_roots
            .iter()
            .zip(&ret_ptrs)
            .filter(|(_, ptr)| !copy_back.iter().any(|(temp, _, _)| temp == *ptr))
            .map(|(root, _)| root.clone())
            .collect::<HashSet<_>>();

        let mut call_args = ret_ptrs.iter().map(|ptr| format!("ptr {ptr}")).collect::<Vec<_>>();

        for (k, (arg, mode)) in args.iter().zip(modes).enumerate() {
            let place = match (mode, arg) {
                (ParamMode::Value, CallArg::Value(operand)) => {
                    let ty = llvm_type(&self.operand_type(*operand));
                    let value = self.operand(*operand);
                    call_args.push(format!("{ty} {value}"));
                    continue;
                }
                (ParamMode::InOut(_), _) => continue,
                (ParamMode::Read | ParamMode::Consume, _) => arg_places[k].clone().unwrap(),
                (ParamMode::Value, CallArg::Place(_)) => unreachable!("scalar passed as a place"),
            };
            let root = self.slot(place.local);

            // Why the argument can't be passed as it is, if it can't.
            let variable = self.describe(place.local, &[]);
            let reason = match mode {
                ParamMode::Read if written.contains(&root) => {
                    Some(format!("the call also writes {variable}"))
                }
                ParamMode::Read => None,
                // Only a whole variable can be handed over.
                _ if !place.path.is_empty() => {
                    Some("only a whole variable can be handed over".to_string())
                }
                _ if self.param_mode(place.local) == Some(ParamMode::Read) => {
                    Some(format!("{variable} is a read-only parameter, owned by the caller"))
                }
                _ if !self.owns(place.local) => {
                    Some(format!("{variable} holds this function's result"))
                }
                _ if count(&read_roots, &root) != 1 => {
                    Some(format!("{variable} is passed to the call more than once"))
                }
                _ if live_after.contains(&place.local) => {
                    Some(format!("{variable} is used again later"))
                }
                _ if count(&dest_roots, &root) != 0 => {
                    Some(format!("the call also writes {variable}"))
                }
                _ => None,
            };

            let (ptr, ty) = self.place_ptr(place.local, &place.path);

            match reason {
                None => call_args.push(format!("ptr {ptr}")),
                Some(reason) => {
                    let temp = self.temp_array(&ty);
                    self.memcpy(&temp, &ptr, &ty);
                    call_args.push(format!("ptr {temp}"));

                    let callee = &self.program.function(func).name;
                    let changes = if *mode == ParamMode::Consume { ", which changes its parameter" } else { "" };
                    let arg = self.describe(place.local, &place.path);
                    self.note(format!("copies {arg} for `{callee}`{changes}: {reason}"), None);
                }
            }
        }

        let name = symbol(&self.program.function(func).name);
        self.line(format!("call void @{name}({})", call_args.join(", ")));

        for (temp, to, ty) in copy_back {
            self.memcpy(&to, &temp, &ty);
        }
    }

    fn assign(&mut self, dest: LocalId, value: &Rvalue) {
        let ty = self.ty(dest);
        let llvm_ty = llvm_type(&ty);

        match value {
            Rvalue::Use(operand) => {
                let slot = self.slot(dest);
                let what = self.describe(dest, &[]);
                self.store_at(&slot, *operand, &ty, &what);
            }

            Rvalue::Binary(op, lhs, rhs) => {
                let value = self.binary(*op, *lhs, *rhs);
                let slot = self.slot(dest);
                self.line(format!("store {llvm_ty} {value}, ptr {slot}"));
            }

            Rvalue::Read(local, path) => {
                let (from, part_ty) = self.place_ptr(*local, path);
                let slot = self.slot(dest);
                let what = format!(
                    "{} into {}",
                    self.describe(*local, path),
                    self.describe(dest, &[])
                );
                self.copy_at(&slot, &from, &part_ty, &what);
            }

            Rvalue::Array(elems) => {
                let Type::Array(elem_ty, _) = &ty else { unreachable!() };

                for (i, elem) in elems.iter().enumerate() {
                    let ptr = self.register();
                    let slot = self.slot(dest);
                    self.line(format!("{ptr} = getelementptr {llvm_ty}, ptr {slot}, i64 0, i64 {i}"));
                    let what = format!("element {i} of {}", self.describe(dest, &[]));
                    self.store_at(&ptr, *elem, elem_ty, &what);
                }
            }

            Rvalue::Struct(fields) => {
                let Type::Struct(id, _) = &ty else { unreachable!() };
                let def = &self.program.structs[id.0];

                for (i, (field, def_field)) in fields.iter().zip(&def.fields).enumerate() {
                    let ptr = self.register();
                    let slot = self.slot(dest);
                    self.line(format!("{ptr} = getelementptr {llvm_ty}, ptr {slot}, i32 0, i32 {i}"));
                    let what = format!("field `{}` of {}", def_field.name, self.describe(dest, &[]));
                    self.store_at(&ptr, *field, &def_field.ty, &what);
                }
            }

            // A loop storing `value` into each element.
            Rvalue::Repeat(value, len) if *len > 0 => {
                let Type::Array(elem_ty, _) = &ty else { unreachable!() };

                let before = self.label.clone();
                let head = self.fresh_label("fill");
                let after = self.fresh_label("filled");
                let (i, ptr, next, done) =
                    (self.register(), self.register(), self.register(), self.register());

                // Load a scalar value once, before the loop.
                let value = match value {
                    Operand::Local(id) if self.ty(*id).is_scalar() => {
                        let register = self.operand(*value);
                        Err(register)
                    }
                    _ => Ok(*value),
                };
                let before = if value.is_err() { self.label.clone() } else { before };

                self.line(format!("br label %{head}"));
                self.start_block(head.clone());
                self.line(format!("{i} = phi i64 [ 0, %{before} ], [ {next}, %{head} ]"));
                let slot = self.slot(dest);
                self.line(format!("{ptr} = getelementptr {llvm_ty}, ptr {slot}, i64 0, i64 {i}"));
                match value {
                    Ok(operand) => {
                        let what = format!("every element of {}", self.describe(dest, &[]));
                        self.store_at(&ptr, operand, elem_ty, &what)
                    }
                    Err(register) => {
                        self.line(format!("store {} {register}, ptr {ptr}", llvm_type(elem_ty)))
                    }
                }
                self.line(format!("{next} = add i64 {i}, 1"));
                self.line(format!("{done} = icmp eq i64 {next}, {len}"));
                self.line(format!("br i1 {done}, label %{after}, label %{head}"));
                self.start_block(after);
            }

            Rvalue::Repeat(..) => {}
        }
    }

    fn binary(&mut self, op: BinOp, lhs: Operand, rhs: Operand) -> String {
        let ty = llvm_type(&self.operand_type(lhs));
        let lhs = self.operand(lhs);
        let rhs = self.operand(rhs);
        let register = self.register();

        let instruction = match op {
            BinOp::Add => "add",
            BinOp::Sub => "sub",
            BinOp::Eq => "icmp eq",
            BinOp::Ne => "icmp ne",
            BinOp::Lt => "icmp slt",
            BinOp::Le => "icmp sle",
            BinOp::Gt => "icmp sgt",
            BinOp::Ge => "icmp sge",
        };

        self.line(format!("{register} = {instruction} {ty} {lhs}, {rhs}"));
        register
    }

    fn terminator(&mut self, terminator: &Terminator) {
        match terminator {
            Terminator::Goto(target) => self.line(format!("br label %bb{}", target.0)),

            Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let cond = self.operand(*cond);
                self.line(format!(
                    "br i1 {cond}, label %bb{}, label %bb{}",
                    then_block.0, else_block.0
                ));
            }

            Terminator::Return => self.line("ret void"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        process::{Command, ExitStatus},
    };

    use super::*;
    use crate::{check, syntax};

    fn tool_available(name: &str) -> bool {
        Command::new(name).arg("--version").output().is_ok()
    }

    fn run(command: &mut Command) {
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{command:?} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Compiles `source` to a native binary with llc and ld.lld, runs it and
    /// returns how it exited. Returns `None` when the tools aren't installed.
    /// Each test passes its own `dir` so tests running in parallel don't
    /// share files.
    fn compile_and_run(dir: &str, name: &str, source: &str) -> Option<ExitStatus> {
        if !tool_available("llc") || !tool_available("ld.lld") {
            eprintln!("skipping: llc or ld.lld not found");
            return None;
        }

        let out = root().join("target/codegen-test").join(dir);
        std::fs::create_dir_all(&out).unwrap();

        let start = out.join("start.o");
        if !start.exists() {
            run(Command::new("llc")
                .args(["--filetype", "obj", "-o"])
                .arg(&start)
                .arg(root().join("stdlib/start.ll")));
        }

        let program = check::check(&syntax::parse(source).unwrap()).unwrap();
        let llvm = emit(&crate::ir::lower(&program), name);

        let ll = out.join(format!("{name}.ll"));
        let obj = out.join(format!("{name}.o"));
        let bin = out.join(name);
        std::fs::write(&ll, llvm).unwrap();

        run(Command::new("llc")
            .args(["--filetype", "obj", "-o"])
            .arg(&obj)
            .arg(&ll));
        run(Command::new("ld.lld").arg("-o").arg(&bin).arg(&obj).arg(&start));

        Some(Command::new(&bin).status().unwrap())
    }

    /// Every program in test/ must exit with 0.
    #[test]
    fn test_programs_exit_with_zero() {
        for entry in std::fs::read_dir(root().join("test")).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_stem().unwrap().to_str().unwrap();
            let source = std::fs::read_to_string(&path).unwrap();

            let Some(status) = compile_and_run("programs", name, &source) else {
                return;
            };
            assert_eq!(status.code(), Some(0), "{name} exited with {status}");
        }
    }

    /// Number of array copies in the emitted code of each named function.
    fn copies(source: &str, functions: &[&str]) -> Vec<usize> {
        let program = check::check(&syntax::parse(source).unwrap()).unwrap();
        let llvm = emit(&crate::ir::lower(&program), "test");

        functions
            .iter()
            .map(|name| {
                let start = llvm.find(&format!("define void @rj_{name}(")).unwrap();
                let end = start + llvm[start..].find("\n}\n").unwrap();
                llvm[start..end].matches("@llvm.memcpy").count()
            })
            .collect()
    }

    const SET: &str = "fn set(a: [i32; 1000], i: i32, v: i32): [i32; 1000] { a[i] = v return a }";

    #[test]
    fn overwriting_call_copies_nothing() {
        let source = format!(
            "{SET}
             fn main(): i32 {{
               a: [i32; 1000]
               i: i32
               a, i = [0; 1000], 0
               while i < 1000 {{
                 a = set(a, i, i)
                 i = i + 1
               }}
               return a[999] - 999
             }}"
        );

        assert_eq!(copies(&source, &["set", "main"]), [0, 0]);
    }

    #[test]
    fn nested_struct_update_copies_nothing() {
        let source = "
            type Node struct { value: i32, edges: [i32; 100] }
            type Graph struct { nodes: [Node; 100] }
            fn bump(g: Graph, i: i32): Graph {
              g.nodes[i].value = g.nodes[i].value + 1
              return g
            }
            fn main(): i32 {
              g: Graph
              i: i32
              g = Graph(nodes: [Node(value: 0, edges: [0; 100]); 100])
              i = 0
              while i < 100 {
                g = bump(g, i)
                i = i + 1
              }
              return g.nodes[99].value - 1
            }";

        // The updates copy nothing. The 3 copies in `main` all come from
        // building `g`: each nested literal is built in a temporary and then
        // copied into its parent (building literals in place would remove
        // them).
        assert_eq!(copies(source, &["bump", "main"]), [0, 3]);
    }

    const GRAPH: &str = "
        type Node struct { value: i32, edges: [i32; 100] }
        type Graph struct { nodes: [Node; 100] }
        fn bump(n: Node): Node { n.value = n.value + 1 return n }
        fn total(edges: [i32; 100]): i32 { return edges[0] + edges[99] }";

    #[test]
    fn updating_a_part_in_place_copies_nothing() {
        let source = format!(
            "{GRAPH}
             fn bump_all(g: Graph): Graph {{
               i: i32
               i = 0
               while i < 100 {{
                 g.nodes[i] = bump(g.nodes[i])
                 i = i + 1
               }}
               return g
             }}
             fn main(): i32 {{ return 0 }}"
        );

        assert_eq!(copies(&source, &["bump", "bump_all"]), [0, 0]);
    }

    #[test]
    fn read_only_part_is_lent() {
        let source = format!(
            "{GRAPH}
             fn sum(g: Graph): i32 {{
               s: i32
               i: i32
               s, i = 0, 0
               while i < 100 {{
                 s = s + total(g.nodes[i].edges)
                 i = i + 1
               }}
               return s
             }}
             fn main(): i32 {{ return 0 }}"
        );

        assert_eq!(copies(&source, &["sum"]), [0]);
    }

    #[test]
    fn result_into_another_part_copies_once() {
        let source = format!(
            "{GRAPH}
             fn shift(g: Graph): Graph {{
               g.nodes[0] = bump(g.nodes[1])
               return g
             }}
             fn main(): i32 {{ return 0 }}"
        );

        // `g.nodes[1]` must survive, and the call reads `g` while writing
        // into it, so the argument goes through a temporary and back.
        assert_eq!(copies(&source, &["shift"]), [2]);
    }

    #[test]
    fn method_calls_copy_nothing() {
        let source = format!(
            "{GRAPH}
             fn Node.bump_node() {{ this.value = this.value + 1 }}
             fn Graph.bump_all() {{
               i: i32
               i = 0
               while i < 100 {{
                 this.nodes[i].bump_node()
                 i = i + 1
               }}
             }}
             fn Graph.first(): i32 {{ return this.nodes[0].value }}
             fn run(g: Graph): i32 {{
               g.bump_all()
               return g.first()
             }}
             fn main(): i32 {{ return 0 }}"
        );

        assert_eq!(
            copies(&source, &["Node.bump_node", "Graph.bump_all", "Graph.first", "run"]),
            [0, 0, 0, 0]
        );
    }

    fn notes(source: &str) -> Vec<String> {
        let program = check::check(&syntax::parse(source).unwrap()).unwrap();
        copy_notes(&crate::ir::lower(&program))
            .into_iter()
            .map(|note| note.message)
            .collect()
    }

    #[test]
    fn copy_notes_explain_kept_copies() {
        let source = format!(
            "{GRAPH}
             fn clobber(n: Node): i32 {{ n.value = 0 return n.value }}
             fn first(n: Node): i32 {{ return n.value }}
             fn run(g: Graph): i32 {{
               g.nodes[0] = bump(g.nodes[0])
               g.nodes[1] = bump(g.nodes[2])
               x: i32
               x = first(g.nodes[3])
               x = x + clobber(g.nodes[4])
               n: Node
               n = g.nodes[5]
               return x + n.value
             }}
             fn main(): i32 {{ return 0 }}"
        );

        assert_eq!(
            notes(&source),
            [
                "copies `g.nodes[2]` in and out of a temporary for `bump`, because the call also \
                 reads `g`, where the result goes",
                "copies `g.nodes[4]` for `clobber`, which changes its parameter: only a whole \
                 variable can be handed over",
                "copies `g.nodes[5]` into `n`",
            ]
        );
    }

    #[test]
    fn copy_notes_give_the_reason_for_consumed_arguments() {
        let source = "
            type B struct { v: [i32; 4] }
            fn clobber(b: B): i32 { b.v[0] = 1 return b.v[0] }
            fn main(): i32 {
              b: B
              b = B(v: [0; 4])
              return clobber(b) + b.v[0] - 1
            }";

        let notes = notes(source);
        assert!(
            notes.iter().any(|n| n == "copies `b` for `clobber`, which changes its parameter: `b` is used again later"),
            "{notes:#?}"
        );
    }

    #[test]
    fn read_only_argument_is_lent() {
        let source = "
            fn first(a: [i32; 1000]): i32 { return a[0] }
            fn main(): i32 {
              a: [i32; 1000]
              a = [0; 1000]
              return first(a) + first(a)
            }";

        assert_eq!(copies(source, &["first", "main"]), [0, 0]);
    }

    #[test]
    fn result_into_other_variable_copies_once() {
        let source = format!(
            "{SET}
             fn main(): i32 {{
               a: [i32; 1000]
               b: [i32; 1000]
               a = [0; 1000]
               b = set(a, 0, 1)
               return a[0] + b[0] - 1
             }}"
        );

        assert_eq!(copies(&source, &["main"]), [1]);
    }

    #[test]
    fn consumed_argument_copied_only_while_needed() {
        let source = "
            fn clobber(a: [i32; 1000]): i32 { a[0] = 1 return a[0] }
            fn main(): i32 {
              a: [i32; 1000]
              x: i32
              a = [0; 1000]
              x = clobber(a)
              x = x + a[0]
              x = x + clobber(a)
              return x - 2
            }";

        // The first call needs a copy because `a` is read afterwards; the
        // second can hand `a` over.
        assert_eq!(copies(source, &["main"]), [1]);
    }

    #[test]
    fn out_of_bounds_index_traps() {
        let source = "
            fn get(a: [i32; 3], i: i32): i32 { return a[i] }
            fn main(): i32 {
              a: [i32; 3]
              a = [0; 3]
              return get(a, 3)
            }";

        let Some(status) = compile_and_run("trap-out-of-bounds", "out_of_bounds", source) else {
            return;
        };

        // Killed by a signal rather than exiting normally.
        assert_eq!(status.code(), None, "expected a trap, got {status}");
    }

    #[test]
    fn negative_index_traps() {
        let source = "
            fn set(a: [i32; 3], i: i32): [i32; 3] { a[i] = 1 return a }
            fn main(): i32 {
              a: [i32; 3]
              a = set([0; 3], 0 - 1)
              return 0
            }";

        let Some(status) = compile_and_run("trap-negative", "negative", source) else {
            return;
        };

        assert_eq!(status.code(), None, "expected a trap, got {status}");
    }
}
