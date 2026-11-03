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

// Spans are kept for diagnostics from later passes.
#![allow(dead_code)]

pub use crate::{
    syntax::ast::BinOp,
    types::{StructDef, StructId, Type},
};

use crate::diagnostic::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuncId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub usize);

#[derive(Debug)]
pub struct Program {
    pub structs: Vec<StructDef>,
    pub functions: Vec<Function>,
    pub main: FuncId,
    pub examples: Vec<Example>,
}

/// An `example`, as a function without parameters that returns the two
/// sides of a comparison (`compare` is set) or a single bool.
#[derive(Debug)]
pub struct Example {
    pub function: Function,
    pub compare: Option<BinOp>,
    pub span: Span,
}

impl Program {
    pub fn function(&self, id: FuncId) -> &Function {
        &self.functions[id.0]
    }
}

#[derive(Debug)]
pub struct Function {
    /// `Type.method` for methods.
    pub name: String,
    /// Methods take the receiver, `this`, as their first parameter.
    pub is_method: bool,
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
    /// All values (and target indices) are evaluated before any target is
    /// written.
    Assign {
        targets: Vec<Place>,
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
    While {
        cond: Expr,
        body: Block,
    },
    Return(Vec<Value>),
}

/// A variable, or a part of one reached through fields and indexes.
#[derive(Debug)]
pub struct Place {
    pub local: LocalId,
    pub path: Vec<PlaceElem>,
}

#[derive(Debug)]
pub enum PlaceElem {
    Index(Expr),
    /// Field number, in declaration order.
    Field(usize),
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
    ArrayList(Vec<Expr>),
    ArrayRepeat(Box<Expr>, u32),
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    /// Field number, in declaration order.
    Field {
        base: Box<Expr>,
        field: usize,
    },
    /// Field values in declaration order.
    StructLit(StructId, Vec<Expr>),
    /// A conversion between types with the same underlying type. Only the
    /// type changes.
    Convert(Box<Expr>),
}

/// A function call, or a method call when `receiver` is set. The receiver
/// is the method's first parameter and is not part of `args`.
#[derive(Debug)]
pub struct Call {
    pub func: FuncId,
    pub receiver: Option<Receiver>,
    pub args: Vec<Value>,
    pub span: Span,
}

#[derive(Debug)]
pub enum Receiver {
    /// A variable or part of one. If the method changes `this`, the result
    /// is written back to it.
    Place(Place),
    /// Any other value. Changes the method makes to `this` are discarded.
    Value(Box<Expr>),
}
