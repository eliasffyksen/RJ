//! Resolved and type-checked program, produced by [`crate::check`].
//!
//! Compared to the AST:
//! - variables are [`LocalId`]s into the function's `locals` table and calls
//!   point at a [`FuncId`], so later stages never look names up;
//! - declarations are gone (they only populate `locals`);
//! - every single-valued expression carries its type;
//! - a call that does not return exactly one value can only appear directly
//!   in a value list (arguments, assignment right-hand side, `return`), where
//!   it contributes all of its return values ([`Value::Multi`]).

// Not consumed yet; lowering to the IR will read these.
#![allow(dead_code)]

pub use crate::syntax::ast::{BinOp, Type};

use crate::diagnostic::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuncId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub usize);

#[derive(Debug)]
pub struct Program {
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
    /// The first `params.len()` locals are the parameters, in order.
    pub params: Vec<LocalId>,
    pub returns: Vec<Type>,
    pub locals: Vec<Local>,
    pub body: Block,
    pub span: Span,
}

impl Function {
    pub fn local(&self, id: LocalId) -> &Local {
        &self.locals[id.0]
    }
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum StmtKind {
    Assign {
        targets: Vec<LocalId>,
        values: Vec<Value>,
    },
    /// A call whose return values are discarded.
    Call(Call),
    If {
        cond: Expr,
        then_block: Block,
        /// Empty when the source had no `else`.
        else_block: Block,
    },
    Return(Vec<Value>),
}

/// One entry of a value list.
#[derive(Debug)]
pub enum Value {
    /// Exactly one value.
    Single(Expr),
    /// A call returning zero or several values, all of which are used.
    Multi(Call),
}

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExprKind {
    Int(i32),
    Bool(bool),
    Local(LocalId),
    /// A call returning exactly one value.
    Call(Call),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

#[derive(Debug)]
pub struct Call {
    pub func: FuncId,
    pub args: Vec<Value>,
    pub span: Span,
}
