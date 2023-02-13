use pest::iterators::Pair;

use crate::{
    ident::{Ident, IdentImpl},
    Rule, scope::{Scopable, ScopeEntry}, expression::Expression, check_rule, unexpected_pair,
};

#[derive(Debug)]
pub enum Stmt {
    VarDecl(VarDecl),
    FuncReturn(Vec<Expression>),
}

impl Stmt {
    pub fn ir(
        &self,
        out: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
    ) -> Result<(), std::io::Error> {
        match self {
            Stmt::VarDecl(var_decl) => var_decl.ir(out, context, scope)?,

            _ => todo!(),
        }

        Ok(())
    }

    pub fn ast(pair: Pair<Rule>) -> Stmt {
        if pair.as_rule() != Rule::stmt {
            panic!("Attempted to generate Stmt from non Stmt pair: {:?}", pair)
        }

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::var_decl => return Self::VarDecl(VarDecl::ast(pair)),
                Rule::func_ret => return Self::ast_return(pair),

                _ => panic!("Unexpected pair: {:?}", pair),
            }
        }

        panic!("No pairs in statement");
    }

    fn ast_return(pair: Pair<Rule>) -> Self {
        check_rule(&pair, Rule::func_ret);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::expr => return Stmt::FuncReturn(Expression::ast(pair)),

                _ => unexpected_pair(&pair),
            }
        }

        panic!("No pair in return statement")
    }
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub ident: Ident,
    pub var_type: Type,
}

impl VarDecl {
    pub fn ast(pair: pest::iterators::Pair<Rule>) -> VarDecl {
        if pair.as_rule() != Rule::var_decl {
            panic!(
                "Attempted to generate VarDecl from non VarDecl pair: {:?}",
                pair
            )
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
            var_type: var_type.expect("No var type"),
        }
    }

    pub fn ir(
        &self,
        out: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
    ) -> Result<(), std::io::Error> {
        let register = context.claim_register();
        writeln!(
            out,
            "  %{} = alloca {}",
            register,
            self.var_type.get_ir_type()
        )?;

        scope.set_entry(ScopeEntry{
            var_decl: self.clone(),
            register,
        });

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Type {
    I32,
}

impl Type {
    pub fn get_ir_type(&self) -> &'static str {
        match self {
            Type::I32 => "i32",
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
