//! Syntax tree produced by the parser.
//!
//! A plain owned tree: every node owns its children and carries the byte
//! span it was parsed from. Later passes that need to attach information to
//! nodes should do so in their own data structures (or by lowering into an
//! IR) rather than by mutating this tree.

// Spans are not read yet; name resolution and type checking will use them.
#![allow(dead_code)]

use std::fmt;

use crate::diagnostic::Span;

#[derive(Debug)]
pub struct Module {
    pub functions: Vec<Function>,
}

#[derive(Debug)]
pub struct Function {
    pub name: Ident,
    pub params: Vec<VarDecl>,
    pub returns: Vec<Type>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug)]
pub struct VarDecl {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum StmtKind {
    Decl(VarDecl),
    Assign {
        targets: Vec<Ident>,
        values: Vec<Expr>,
    },
    Call(Call),
    If {
        cond: Expr,
        then_block: Block,
        else_block: Option<Block>,
    },
    Return(Vec<Expr>),
}

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExprKind {
    Int(i32),
    Bool(bool),
    Var(Ident),
    Call(Call),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

#[derive(Debug)]
pub struct Call {
    pub callee: Ident,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    I32,
    Bool,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Type::I32 => "i32",
            Type::Bool => "bool",
        })
    }
}
