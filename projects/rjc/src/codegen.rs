//! Emits textual LLVM IR from the RJ IR.
//!
//! Calling convention (matching `stdlib/start.ll`): every function returns
//! `void` and takes one pointer per return value first, then its arguments.
//! Scalar arguments are passed by value. Array arguments are passed as a
//! pointer to a copy the caller makes for the call, which the callee then
//! owns and uses as that parameter's storage. A call passes the destination
//! locals' storage as the return pointers, so `x = f(x)` hands `f` a pointer
//! to `x` itself.
//!
//! Every other local gets a stack slot (`alloca`) in the entry block. LLVM's
//! `mem2reg` turns scalar slots into registers when optimising.
//!
//! Array indexing is bounds checked; an out-of-bounds index calls
//! `llvm.trap`, which kills the program.
//!
//! Functions are emitted as `rj_<name>` so they can't collide with symbols
//! from other objects (libc's `exit`, for example).

use std::fmt::Write as _;

use crate::ir::*;

pub fn emit(program: &Program, source_name: &str) -> String {
    let mut out = String::new();

    writeln!(out, "source_filename = \"{}\"", source_name.escape_default()).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)").unwrap();
    writeln!(out, "declare void @llvm.trap()").unwrap();

    for function in &program.functions {
        writeln!(out).unwrap();
        out.push_str(&FunctionEmitter::new(program, function).emit());
    }

    out
}

pub fn symbol(name: &str) -> String {
    format!("rj_{name}")
}

fn llvm_type(ty: Type) -> String {
    match ty {
        Type::I32 => "i32".to_string(),
        Type::Bool => "i1".to_string(),
        Type::Array(elem, len) => format!("[{len} x {}]", llvm_type(elem.ty())),
    }
}

/// Size in bytes, as laid out in memory.
fn size_of(ty: Type) -> u64 {
    match ty {
        Type::I32 => 4,
        Type::Bool => 1,
        Type::Array(elem, len) => size_of(elem.ty()) * u64::from(len),
    }
}

fn is_array(ty: Type) -> bool {
    matches!(ty, Type::Array(..))
}

/// Name of the pointer to a local's storage.
fn slot(id: LocalId) -> String {
    format!("%l{}", id.0)
}

struct FunctionEmitter<'a> {
    program: &'a Program,
    function: &'a Function,
    /// Allocas, placed in the entry block so loops don't grow the stack.
    entry: String,
    body: String,
    /// Label of the LLVM block currently being written.
    label: String,
    next_register: usize,
    next_label: usize,
    uses_trap: bool,
}

impl<'a> FunctionEmitter<'a> {
    fn new(program: &'a Program, function: &'a Function) -> Self {
        FunctionEmitter {
            program,
            function,
            entry: String::new(),
            body: String::new(),
            label: String::new(),
            next_register: 0,
            next_label: 0,
            uses_trap: false,
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
        self.function.local(id).ty
    }

    fn emit(mut self) -> String {
        let function = self.function;

        let mut params = vec![];
        for ret in &function.returns {
            params.push(format!("ptr {}", slot(*ret)));
        }
        for param in &function.params {
            let ty = self.ty(*param);

            if is_array(ty) {
                params.push(format!("ptr {}", slot(*param)));
            } else {
                params.push(format!("{} %p{}", llvm_type(ty), param.0));
            }
        }

        for (i, local) in function.locals.iter().enumerate() {
            let id = LocalId(i);
            let is_array_param = function.params.contains(&id) && is_array(local.ty);

            if local.kind != LocalKind::Return && !is_array_param {
                writeln!(self.entry, "  {} = alloca {}", slot(id), llvm_type(local.ty)).unwrap();
            }
        }

        for param in &function.params {
            let ty = self.ty(*param);

            if !is_array(ty) {
                writeln!(
                    self.entry,
                    "  store {} %p{}, ptr {}",
                    llvm_type(ty),
                    param.0,
                    slot(*param)
                )
                .unwrap();
            }
        }

        for (i, block) in function.blocks.iter().enumerate() {
            self.start_block(format!("bb{i}"));

            for stmt in &block.stmts {
                self.stmt(stmt);
            }

            self.terminator(&block.terminator);
        }

        if self.uses_trap {
            self.start_block("trap".to_string());
            self.line("call void @llvm.trap()");
            self.line("unreachable");
        }

        format!(
            "define void @{}({}) {{\nentry:\n{}  br label %bb0\n{}}}\n",
            symbol(&function.name),
            params.join(", "),
            self.entry,
            self.body
        )
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
                debug_assert!(!is_array(ty), "array used as a scalar operand");

                let register = self.register();
                self.line(format!("{register} = load {}, ptr {}", llvm_type(ty), slot(id)));
                register
            }
            Operand::Int(value) => value.to_string(),
            Operand::Bool(value) => value.to_string(),
        }
    }

    fn memcpy(&mut self, dest: &str, src: &str, ty: Type) {
        self.line(format!(
            "call void @llvm.memcpy.p0.p0.i64(ptr {dest}, ptr {src}, i64 {}, i1 false)",
            size_of(ty)
        ));
    }

    /// Bounds-checks `index` and returns a pointer to that element.
    fn element_ptr(&mut self, array: LocalId, index: Operand) -> String {
        let ty = self.ty(array);
        let Type::Array(_, len) = ty else {
            unreachable!("indexing a non-array local");
        };

        let index = self.operand(index);
        let in_bounds = self.register();
        let next = self.fresh_label("inbounds");

        // Unsigned comparison also rejects negative indices.
        self.line(format!("{in_bounds} = icmp ult i32 {index}, {len}"));
        self.line(format!("br i1 {in_bounds}, label %{next}, label %trap"));
        self.uses_trap = true;
        self.start_block(next);

        let ptr = self.register();
        self.line(format!(
            "{ptr} = getelementptr {}, ptr {}, i64 0, i32 {index}",
            llvm_type(ty),
            slot(array)
        ));
        ptr
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assign { dest, value } if is_array(self.ty(*dest)) => {
                self.array_assign(*dest, value)
            }

            Stmt::Assign { dest, value } => {
                let value = match value {
                    Rvalue::Use(operand) => self.operand(*operand),
                    Rvalue::Binary(op, lhs, rhs) => self.binary(*op, *lhs, *rhs),
                    Rvalue::Index(array, index) => {
                        let ptr = self.element_ptr(*array, *index);
                        let register = self.register();
                        let ty = llvm_type(self.ty(*dest));
                        self.line(format!("{register} = load {ty}, ptr {ptr}"));
                        register
                    }
                    Rvalue::Array(_) | Rvalue::Repeat(..) => {
                        unreachable!("array value assigned to a scalar")
                    }
                };
                let ty = llvm_type(self.ty(*dest));

                self.line(format!("store {ty} {value}, ptr {}", slot(*dest)));
            }

            Stmt::SetIndex {
                array,
                index,
                value,
            } => {
                let ty = llvm_type(self.operand_type(*value));
                let value = self.operand(*value);
                let ptr = self.element_ptr(*array, *index);

                self.line(format!("store {ty} {value}, ptr {ptr}"));
            }

            Stmt::Call { func, args, dests } => {
                let mut call_args = dests
                    .iter()
                    .map(|dest| format!("ptr {}", slot(*dest)))
                    .collect::<Vec<_>>();

                for arg in args {
                    let ty = self.operand_type(*arg);

                    if let (Type::Array(..), Operand::Local(src)) = (ty, arg) {
                        // The callee owns its array arguments, so give it a
                        // copy.
                        let copy = format!("%copy{}", self.fresh_label(""));
                        writeln!(self.entry, "  {copy} = alloca {}", llvm_type(ty)).unwrap();
                        self.memcpy(&copy, &slot(*src), ty);
                        call_args.push(format!("ptr {copy}"));
                    } else {
                        let value = self.operand(*arg);
                        call_args.push(format!("{} {value}", llvm_type(ty)));
                    }
                }

                let name = symbol(&self.program.function(*func).name);
                self.line(format!("call void @{name}({})", call_args.join(", ")));
            }
        }
    }

    fn array_assign(&mut self, dest: LocalId, value: &Rvalue) {
        let ty = self.ty(dest);
        let Type::Array(elem, len) = ty else {
            unreachable!()
        };
        let elem_ty = llvm_type(elem.ty());
        let array_ty = llvm_type(ty);

        match value {
            Rvalue::Use(Operand::Local(src)) => {
                if *src != dest {
                    self.memcpy(&slot(dest), &slot(*src), ty);
                }
            }

            Rvalue::Array(elems) => {
                for (i, elem) in elems.iter().enumerate() {
                    let value = self.operand(*elem);
                    let ptr = self.register();
                    self.line(format!(
                        "{ptr} = getelementptr {array_ty}, ptr {}, i64 0, i64 {i}",
                        slot(dest)
                    ));
                    self.line(format!("store {elem_ty} {value}, ptr {ptr}"));
                }
            }

            // A loop storing `value` into each element.
            Rvalue::Repeat(value, len) if *len > 0 => {
                let value = self.operand(*value);
                let before = self.label.clone();
                let head = self.fresh_label("fill");
                let after = self.fresh_label("filled");
                let (i, ptr, next, done) =
                    (self.register(), self.register(), self.register(), self.register());

                self.line(format!("br label %{head}"));
                self.start_block(head.clone());
                self.line(format!("{i} = phi i64 [ 0, %{before} ], [ {next}, %{head} ]"));
                self.line(format!(
                    "{ptr} = getelementptr {array_ty}, ptr {}, i64 0, i64 {i}",
                    slot(dest)
                ));
                self.line(format!("store {elem_ty} {value}, ptr {ptr}"));
                self.line(format!("{next} = add i64 {i}, 1"));
                self.line(format!("{done} = icmp eq i64 {next}, {len}"));
                self.line(format!("br i1 {done}, label %{after}, label %{head}"));
                self.start_block(after);
            }

            Rvalue::Repeat(..) => {}

            _ => unreachable!("invalid array assignment {value} of length {len}"),
        }
    }

    fn binary(&mut self, op: BinOp, lhs: Operand, rhs: Operand) -> String {
        let ty = llvm_type(self.operand_type(lhs));
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
