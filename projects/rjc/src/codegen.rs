//! Emits textual LLVM IR from the RJ IR.
//!
//! Calling convention (matching `stdlib/start.ll`): every function returns
//! `void`, takes one pointer per return value first, then its arguments by
//! value. A call passes the destination locals' storage as the return
//! pointers, so `x = f(x)` hands `f` a pointer to `x` itself.
//!
//! Every local gets a stack slot (`alloca`); return locals use the incoming
//! return pointers instead. LLVM's `mem2reg` turns the slots into registers
//! when optimising.
//!
//! Functions are emitted as `rj_<name>` so they can't collide with symbols
//! from other objects (libc's `exit`, for example).

use std::fmt::Write as _;

use crate::ir::*;

pub fn emit(program: &Program, source_name: &str) -> String {
    let mut out = String::new();

    writeln!(out, "source_filename = \"{}\"", source_name.escape_default()).unwrap();

    for function in &program.functions {
        writeln!(out).unwrap();
        FunctionEmitter {
            program,
            function,
            out: &mut out,
            next_register: 0,
        }
        .emit();
    }

    out
}

pub fn symbol(name: &str) -> String {
    format!("rj_{name}")
}

fn llvm_type(ty: Type) -> &'static str {
    match ty {
        Type::I32 => "i32",
        Type::Bool => "i1",
    }
}

/// Name of the pointer to a local's storage.
fn slot(id: LocalId) -> String {
    format!("%l{}", id.0)
}

fn block_label(id: BlockId) -> String {
    format!("bb{}", id.0)
}

struct FunctionEmitter<'a> {
    program: &'a Program,
    function: &'a Function,
    out: &'a mut String,
    next_register: usize,
}

impl FunctionEmitter<'_> {
    fn line(&mut self, line: impl AsRef<str>) {
        self.out.push_str("  ");
        self.out.push_str(line.as_ref());
        self.out.push('\n');
    }

    fn register(&mut self) -> String {
        self.next_register += 1;
        format!("%r{}", self.next_register)
    }

    fn emit(mut self) {
        let function = self.function;

        let mut params = vec![];
        for ret in &function.returns {
            params.push(format!("ptr {}", slot(*ret)));
        }
        for param in &function.params {
            let ty = llvm_type(function.local(*param).ty);
            params.push(format!("{ty} %p{}", param.0));
        }

        writeln!(
            self.out,
            "define void @{}({}) {{",
            symbol(&function.name),
            params.join(", ")
        )
        .unwrap();
        writeln!(self.out, "entry:").unwrap();

        for (i, local) in function.locals.iter().enumerate() {
            if local.kind != LocalKind::Return {
                self.line(format!("{} = alloca {}", slot(LocalId(i)), llvm_type(local.ty)));
            }
        }

        for param in &function.params {
            let ty = llvm_type(function.local(*param).ty);
            self.line(format!("store {ty} %p{}, ptr {}", param.0, slot(*param)));
        }

        self.line(format!("br label %{}", block_label(BlockId(0))));

        for (i, block) in function.blocks.iter().enumerate() {
            writeln!(self.out, "{}:", block_label(BlockId(i))).unwrap();

            for stmt in &block.stmts {
                self.stmt(stmt);
            }

            self.terminator(&block.terminator);
        }

        writeln!(self.out, "}}").unwrap();
    }

    fn operand_type(&self, operand: Operand) -> Type {
        match operand {
            Operand::Local(id) => self.function.local(id).ty,
            Operand::Int(_) => Type::I32,
            Operand::Bool(_) => Type::Bool,
        }
    }

    /// Returns the LLVM value of an operand, loading locals.
    fn operand(&mut self, operand: Operand) -> String {
        match operand {
            Operand::Local(id) => {
                let register = self.register();
                let ty = llvm_type(self.function.local(id).ty);
                self.line(format!("{register} = load {ty}, ptr {}", slot(id)));
                register
            }
            Operand::Int(value) => value.to_string(),
            Operand::Bool(value) => value.to_string(),
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assign { dest, value } => {
                let value = match value {
                    Rvalue::Use(operand) => self.operand(*operand),
                    Rvalue::Binary(op, lhs, rhs) => self.binary(*op, *lhs, *rhs),
                };
                let ty = llvm_type(self.function.local(*dest).ty);

                self.line(format!("store {ty} {value}, ptr {}", slot(*dest)));
            }

            Stmt::Call { func, args, dests } => {
                let mut call_args = dests
                    .iter()
                    .map(|dest| format!("ptr {}", slot(*dest)))
                    .collect::<Vec<_>>();

                for arg in args {
                    let ty = llvm_type(self.operand_type(*arg));
                    let value = self.operand(*arg);
                    call_args.push(format!("{ty} {value}"));
                }

                let name = symbol(&self.program.function(*func).name);
                self.line(format!("call void @{name}({})", call_args.join(", ")));
            }
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
            Terminator::Goto(target) => self.line(format!("br label %{}", block_label(*target))),

            Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let cond = self.operand(*cond);
                self.line(format!(
                    "br i1 {cond}, label %{}, label %{}",
                    block_label(*then_block),
                    block_label(*else_block)
                ));
            }

            Terminator::Return => self.line("ret void"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        process::Command,
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

    /// Compiles every program in test/ to a native binary with llc and
    /// ld.lld and checks that it exits with 0. Skipped when the tools are
    /// not installed.
    #[test]
    fn test_programs_compile_and_exit_with_zero() {
        if !tool_available("llc") || !tool_available("ld.lld") {
            eprintln!("skipping: llc or ld.lld not found");
            return;
        }

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = root.join("target/codegen-test");
        std::fs::create_dir_all(&out).unwrap();

        let start = out.join("start.o");
        run(Command::new("llc")
            .args(["--filetype", "obj", "-o"])
            .arg(&start)
            .arg(root.join("stdlib/start.ll")));

        for entry in std::fs::read_dir(root.join("test")).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_stem().unwrap().to_str().unwrap();
            let source = std::fs::read_to_string(&path).unwrap();

            let program = check::check(&syntax::parse(&source).unwrap()).unwrap();
            let llvm = emit(&crate::ir::lower(&program), &path.display().to_string());

            let ll = out.join(format!("{name}.ll"));
            let obj = out.join(format!("{name}.o"));
            let bin = out.join(name);
            std::fs::write(&ll, llvm).unwrap();

            run(Command::new("llc")
                .args(["--filetype", "obj", "-o"])
                .arg(&obj)
                .arg(&ll));
            run(Command::new("ld.lld").arg("-o").arg(&bin).arg(&obj).arg(&start));

            let status = Command::new(&bin).status().unwrap();
            assert_eq!(status.code(), Some(0), "{name} exited with {status}");
        }
    }
}
