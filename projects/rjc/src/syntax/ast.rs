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
    pub types: Vec<TypeDef>,
    pub functions: Vec<Function>,
    pub examples: Vec<Example>,
}

/// `example expr`: a bool expression, or a comparison of any two values,
/// that must hold.
#[derive(Debug)]
pub struct Example {
    pub expr: Expr,
    pub span: Span,
}

/// `type Name struct { ... }` or `type Name <type>`.
#[derive(Debug)]
pub struct TypeDef {
    pub name: Ident,
    pub body: TypeDefBody,
    pub span: Span,
}

#[derive(Debug)]
pub enum TypeDefBody {
    Struct(Vec<VarDecl>),
    Type(TypeExpr),
}

#[derive(Debug)]
pub struct Function {
    /// The type `T` of a method `fn T.name(...)`.
    pub receiver: Option<Ident>,
    pub name: Ident,
    pub params: Vec<VarDecl>,
    pub returns: Vec<TypeExpr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: Ident,
    pub ty: TypeExpr,
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
        targets: Vec<Place>,
        values: Vec<Expr>,
    },
    Call(Call),
    If {
        cond: Expr,
        then_block: Block,
        else_block: Option<Block>,
    },
    While {
        cond: Expr,
        body: Block,
    },
    Return(Vec<Expr>),
    /// A method call used as a statement.
    Expr(Expr),
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
    /// `[a, b, c]`
    ArrayList(Vec<Expr>),
    /// `[value; len]`
    ArrayRepeat(Box<Expr>, u32),
    /// `base[index]`
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    /// `base.field`
    Field {
        base: Box<Expr>,
        field: Ident,
    },
    /// `receiver.method(args)`
    MethodCall {
        receiver: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
    },
    /// `Name(field: value, ...)`
    StructLit {
        name: Ident,
        fields: Vec<(Ident, Expr)>,
    },
}

/// An assignment target: a variable, or a part of one such as `x.items[i]`.
#[derive(Debug)]
pub struct Place {
    pub name: Ident,
    pub path: Vec<PlaceElem>,
    pub span: Span,
}

#[derive(Debug)]
pub enum PlaceElem {
    Index(Expr),
    Field(Ident),
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

/// A type as written in the source. The checker resolves it to a
/// [`crate::types::Type`].
#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeExprKind {
    I32,
    Bool,
    /// `[elem; len]`
    Array(Box<TypeExpr>, u32),
    /// A struct name.
    Named(Ident),
}

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TypeExprKind::I32 => f.write_str("i32"),
            TypeExprKind::Bool => f.write_str("bool"),
            TypeExprKind::Array(elem, len) => write!(f, "[{elem}; {len}]"),
            TypeExprKind::Named(name) => f.write_str(&name.name),
        }
    }
}
