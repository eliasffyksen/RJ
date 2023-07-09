use std::fmt::{Display, Debug};

#[derive(Debug, Hash)]
pub enum Type {
    Unknown,
    I32,
}

#[derive(Hash)]
pub struct TypeList {
    pub list: Vec<Type>,
}

impl Type {
    pub fn from_str(_type: &'_ str) -> Type {
        match _type {
            "i32" => Type::I32,

            _ => panic!("invalid type string"),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Type::Unknown => "unknown",
            Type::I32 => "i32",
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Display for TypeList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, item) in self.list.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }

            write!(f, "{}", item.as_str())?;
        }

        Ok(())
    }
}

impl Debug for TypeList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.list.fmt(f)
    }
}
