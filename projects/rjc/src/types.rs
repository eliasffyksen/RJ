//! Resolved types, shared by the checker, the IR and code generation.

use std::{fmt, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructId(pub usize);

/// A named type that isn't a struct, like `type NodeId i32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NamedId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    I32,
    Bool,
    /// `[elem; len]`
    Array(Box<Type>, u32),
    /// A struct, with its name kept for messages.
    Struct(StructId, Rc<str>),
    /// A named type that isn't a struct. Only exists while checking: the
    /// checker replaces it with its underlying type in its output, so later
    /// stages never see it.
    Named(NamedId, Rc<str>),
}

impl Type {
    /// Only meaningful after checking, when no `Named` types remain.
    pub fn is_scalar(&self) -> bool {
        debug_assert!(!matches!(self, Type::Named(..)), "unresolved named type");
        matches!(self, Type::I32 | Type::Bool)
    }

    /// Arrays and structs: stored in memory and passed by pointer.
    pub fn is_aggregate(&self) -> bool {
        !self.is_scalar()
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I32 => f.write_str("i32"),
            Type::Bool => f.write_str("bool"),
            Type::Array(elem, len) => write!(f, "[{elem}; {len}]"),
            Type::Struct(_, name) | Type::Named(_, name) => f.write_str(name),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

impl StructDef {
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|field| field.name == name)
    }
}
