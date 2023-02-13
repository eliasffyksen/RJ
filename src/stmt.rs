use crate::{ident::{Ident, IdentImpl}, Rule};

#[derive(Debug)]
pub enum Stmt {
    VarDecl(VarDecl),
}

impl Stmt {
    pub fn ir(&self, out: &mut impl std::io::Write, context: &mut crate::IRContext) -> Result<(), std::io::Error> {
        match self {
            Stmt::VarDecl(var_decl) => var_decl.ir(out, context)?
        }

        Ok(())
    }

    pub fn ast(pair: pest::iterators::Pair<Rule>) -> Stmt {
        if pair.as_rule() != Rule::stmt {
            panic!("Attempted to generate Stmt from non Stmt pair: {:?}", pair)
        }

        let mut stmt = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::var_decl => stmt = Some(Stmt::VarDecl(VarDecl::ast(pair))),

                _ => panic!("Unexpected pair: {:?}", pair),
            }
        }

        stmt.expect("No valid statement")
    }
}

#[derive(Debug)]
struct VarDecl {
    ident: Ident,
    var_type: Type,
}

impl VarDecl {
    pub fn ast(pair: pest::iterators::Pair<Rule>) -> VarDecl {
        if pair.as_rule() != Rule::var_decl {
            panic!("Attempted to generate VarDecl from non VarDecl pair: {:?}", pair)
        }

        let mut ident = None;
        let mut var_type = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => ident = Some(Ident::ast(pair)),
                Rule::var_type => var_type = Some(Type::ast(pair)),

                _ => panic!("Unexpected pair: {:?}", pair),
            }
        }

        VarDecl {
            ident: ident.expect("No identifier"),
            var_type: var_type.expect("No var type")
        }
    }

    pub fn ir(&self, out: &mut impl std::io::Write, context: &mut crate::IRContext) -> Result<(), std::io::Error> {
        let register = context.claim_register();
        writeln!(out, "  %{} = alloca {}", register, self.var_type.get_ir_type())?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum Type {
    I32,
}

impl Type {
    pub fn get_ir_type(&self) -> &'static str {
        match self {
            Type::I32 => "i32"
        }
    }

    pub fn ast(pair: pest::iterators::Pair<Rule>) -> Type {
        if pair.as_rule() != Rule::var_type {
            panic!("Attempted to generate Type from non Type pair: {:?}", pair)
        }

        match pair.as_str() {
            "i32" => Type::I32,

            _ => panic!("Unknown type {:?}", pair),
        }
    }
}
