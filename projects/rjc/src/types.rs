//! Resolved types, shared by the checker, the IR and code generation.

use std::{fmt, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    I32,
    Bool,
    /// `[elem; len]`
    Array(Box<Type>, u32),
    /// A struct, with its name kept for messages.
    Struct(StructId, Rc<str>),
}

impl Type {
    pub fn is_scalar(&self) -> bool {
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
            Type::Struct(_, name) => f.write_str(name),
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
