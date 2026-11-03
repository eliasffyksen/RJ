//! RJ IR: each function is a control-flow graph of basic blocks operating on
//! a flat table of locals.
//!
//! - Parameters, return slots, user variables and temporaries are all locals.
//! - Statements write whole locals (`dest = ...` or a call writing its
//!   `dests`), except `local.path = value`, which updates one part of an
//!   array or struct local in place. Operands only ever read locals, and
//!   reading an array or struct local copies it. That makes def/use and
//!   liveness of every value explicit, which is what the parameter-mode
//!   analysis needs: `x = f(x)` appears as `call f(x) -> [x]`.
//! - A call reads all of its arguments before writing any of its `dests`.
//! - Returning means assigning the return slots and then `return`.

// Some accessors are only used by tests until the analysis and backend exist.
#![allow(dead_code)]

mod lower;
#[cfg(test)]
mod interp;
pub mod liveness;
pub mod modes;

use std::fmt;

pub use crate::hir::{BinOp, FuncId, LocalId, StructDef, Type};
pub use lower::lower;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

#[derive(Debug)]
pub struct Program {
    pub structs: Vec<StructDef>,
    pub functions: Vec<Function>,
    pub main: FuncId,
}

impl Program {
    pub fn function(&self, id: FuncId) -> &Function {
        &self.functions[id.0]
    }
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<LocalId>,
    /// One local per return value; `return` hands back their contents.
    pub returns: Vec<LocalId>,
    pub locals: Vec<Local>,
    /// `blocks[0]` is the entry block.
    pub blocks: Vec<BasicBlock>,
}

impl Function {
    pub fn local(&self, id: LocalId) -> &Local {
        &self.locals[id.0]
    }

    pub fn block(&self, id: BlockId) -> &BasicBlock {
        &self.blocks[id.0]
    }
}

#[derive(Debug)]
pub struct Local {
    pub ty: Type,
    pub kind: LocalKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalKind {
    Param(String),
    Return,
    Var(String),
    Temp,
}

#[derive(Debug)]
pub struct BasicBlock {
    pub stmts: Vec<Stmt>,
    pub terminator: Terminator,
}

#[derive(Debug)]
pub enum Stmt {
    Assign {
        dest: LocalId,
        value: Rvalue,
    },
    Call {
        func: FuncId,
        args: Vec<Operand>,
        dests: Vec<LocalId>,
    },
    /// `local.path = value`: updates one part, keeping the rest. The path
    /// is never empty.
    SetPlace {
        local: LocalId,
        path: Vec<Elem>,
        value: Operand,
    },
}

/// One step of a path into an array or struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Elem {
    Index(Operand),
    /// Field number, in declaration order.
    Field(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    /// Reads the current value of a local.
    Local(LocalId),
    Int(i32),
    Bool(bool),
}

#[derive(Debug)]
pub enum Rvalue {
    Use(Operand),
    Binary(BinOp, Operand, Operand),
    /// `[a, b, c]`
    Array(Vec<Operand>),
    /// `[value; len]`
    Repeat(Operand, u32),
    /// Field values in declaration order.
    Struct(Vec<Operand>),
    /// `local.path`: reads a part of a local. The path is never empty.
    Read(LocalId, Vec<Elem>),
}

#[derive(Debug)]
pub enum Terminator {
    Goto(BlockId),
    Branch {
        cond: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return,
}

impl Terminator {
    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Terminator::Goto(target) => vec![*target],
            Terminator::Branch {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
            Terminator::Return => vec![],
        }
    }
}

// Text form, printed by `--emit-ir`:
//
//   fn fib(_0: i32) -> (_1: i32) {
//       _0: i32 // param n
//       _1: i32 // return
//       _2: bool
//
//   bb0:
//       _2 = _0 < 3
//       branch _2, bb1, bb2
//   ...

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, function) in self.functions.iter().enumerate() {
            if i != 0 {
                writeln!(f)?;
            }
            function.fmt_with(f, self)?;
        }
        Ok(())
    }
}

impl Function {
    fn fmt_with(&self, f: &mut fmt::Formatter<'_>, program: &Program) -> fmt::Result {
        let typed = |ids: &[LocalId]| {
            ids.iter()
                .map(|id| format!("{id}: {}", self.local(*id).ty))
                .collect::<Vec<_>>()
                .join(", ")
        };

        writeln!(
            f,
            "fn {}({}) -> ({}) {{",
            self.name,
            typed(&self.params),
            typed(&self.returns)
        )?;

        for (param, mode) in self.params.iter().zip(modes::param_modes(self)) {
            if mode != modes::ParamMode::Value {
                writeln!(f, "    // {param}: {mode}")?;
            }
        }

        for (i, local) in self.locals.iter().enumerate() {
            write!(f, "    {}: {}", LocalId(i), local.ty)?;
            match &local.kind {
                LocalKind::Param(name) => writeln!(f, " // param {name}")?,
                LocalKind::Return => writeln!(f, " // return")?,
                LocalKind::Var(name) => writeln!(f, " // {name}")?,
                LocalKind::Temp => writeln!(f)?,
            }
        }

        for (i, block) in self.blocks.iter().enumerate() {
            writeln!(f, "\n{}:", BlockId(i))?;

            for stmt in &block.stmts {
                match stmt {
                    Stmt::Assign {
                        dest,
                        value: Rvalue::Read(local, path),
                    } => writeln!(f, "    {dest} = {}", self.place_str(program, *local, path))?,
                    Stmt::Assign { dest, value } => writeln!(f, "    {dest} = {value}")?,
                    Stmt::SetPlace { local, path, value } => {
                        writeln!(f, "    {} = {value}", self.place_str(program, *local, path))?
                    }
                    Stmt::Call { func, args, dests } => {
                        let args = args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
                        let dests = dests.iter().map(|d| d.to_string()).collect::<Vec<_>>();
                        writeln!(
                            f,
                            "    call {}({}) -> [{}]",
                            program.function(*func).name,
                            args.join(", "),
                            dests.join(", ")
                        )?;
                    }
                }
            }

            match &block.terminator {
                Terminator::Goto(target) => writeln!(f, "    goto {target}")?,
                Terminator::Branch {
                    cond,
                    then_block,
                    else_block,
                } => writeln!(f, "    branch {cond}, {then_block}, {else_block}")?,
                Terminator::Return => writeln!(f, "    return")?,
            }
        }

        writeln!(f, "}}")
    }
}

impl Function {
    /// `_1.items[_2].x`, with field names looked up from the types.
    fn place_str(&self, program: &Program, local: LocalId, path: &[Elem]) -> String {
        let mut out = local.to_string();
        let mut ty = &self.local(local).ty;

        for elem in path {
            match (elem, ty) {
                (Elem::Index(index), Type::Array(elem, _)) => {
                    out += &format!("[{index}]");
                    ty = elem;
                }
                (Elem::Field(field), Type::Struct(id, _)) => {
                    let field = &program.structs[id.0].fields[*field];
                    out += &format!(".{}", field.name);
                    ty = &field.ty;
                }
                _ => unreachable!("path does not match type {ty}"),
            }
        }

        out
    }
}

impl fmt::Display for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_{}", self.0)
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Local(id) => write!(f, "{id}"),
            Operand::Int(value) => write!(f, "{value}"),
            Operand::Bool(value) => write!(f, "{value}"),
        }
    }
}

impl fmt::Display for Rvalue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rvalue::Use(operand) => write!(f, "{operand}"),
            Rvalue::Binary(op, lhs, rhs) => write!(f, "{lhs} {} {rhs}", op.as_str()),
            Rvalue::Array(elems) => {
                let elems = elems.iter().map(|e| e.to_string()).collect::<Vec<_>>();
                write!(f, "[{}]", elems.join(", "))
            }
            Rvalue::Repeat(value, len) => write!(f, "[{value}; {len}]"),
            Rvalue::Struct(fields) => {
                let fields = fields.iter().map(|e| e.to_string()).collect::<Vec<_>>();
                write!(f, "{{{}}}", fields.join(", "))
            }
            Rvalue::Read(local, path) => {
                write!(f, "{local}")?;
                for elem in path {
                    match elem {
                        Elem::Index(index) => write!(f, "[{index}]")?,
                        Elem::Field(field) => write!(f, ".{field}")?,
                    }
                }
                Ok(())
            }
        }
    }
}
