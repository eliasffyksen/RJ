
#[derive(Debug)]
pub enum Type {
    Unknown,
    I32,
}

impl Type {
    pub fn from_str(_type :&'_ str) -> Type {
        match _type {
            "i32" => Type::I32,

            _ => panic!("invalid type string"),
        }
    }
}
