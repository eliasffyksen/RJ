use pest::iterators::Pair;
use std::fmt::Write as _;

use crate::{
    check_rule,
    expression::{ExpressionInput, ExpressionList},
    ident::{Ident, IdentImpl},
    scope::{Scopable, ScopeEntry, ScopeVariable},
    unexpected_pair, Rule,
};

#[derive(Debug)]
pub enum Stmt {
    VarDecl(VarDecl),
    FuncReturn(ExpressionList),
    Assign((Vec<Ident>, ExpressionList)),
}

impl Stmt {
    pub fn ast(pair: Pair<Rule>) -> Stmt {
        if pair.as_rule() != Rule::stmt {
            panic!("Attempted to generate Stmt from non Stmt pair: {:?}", pair)
        }

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::var_decl => return Self::VarDecl(VarDecl::ast(pair)),
                Rule::func_ret => return Self::ast_return(pair),
                Rule::assign => return Self::ast_assign(pair),

                _ => panic!("Unexpected pair: {:?}", pair),
            }
        }

        panic!("No pairs in statement");
    }

    fn ast_return(pair: Pair<Rule>) -> Self {
        check_rule(&pair, Rule::func_ret);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::expr_list => return Stmt::FuncReturn(ExpressionList::ast(pair)),

                _ => unexpected_pair(&pair),
            }
        }

        panic!("No pair in return statement")
    }

    fn ast_assign(pair: Pair<Rule>) -> Self {
        check_rule(&pair, Rule::assign);

        let mut identifiers = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::expr_list => return Self::Assign((identifiers, ExpressionList::ast(pair))),
                Rule::ident => identifiers.push(Ident::ast(pair)),

                _ => unexpected_pair(&pair),
            }
        }

        panic!("No expression in return statement")
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
    ) -> Result<(), std::io::Error> {
        match self {
            Stmt::VarDecl(var_decl) => {
                var_decl.ir(output, context, scope)?;
            }

            Stmt::FuncReturn(func_return) => Self::ir_return(func_return, output, context, scope)?,

            Stmt::Assign((identifiers, expressions)) => {
                Self::ir_assign(identifiers, expressions, output, context, scope)?
            }
        }

        writeln!(output)?;

        Ok(())
    }

    pub fn ir_return(
        func_return: &ExpressionList,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
    ) -> Result<(), std::io::Error> {
        let mut ret_type = scope
            .get_ret_type()
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut store_to = String::new();
                write!(&mut store_to, "{}* %{}", t.get_ir_type(), i)?;

                let result: Result<_, std::fmt::Error> = Ok(ExpressionInput {
                    data_type: t.clone(),
                    store_to: Some(store_to),
                });

                result
            })
            .try_collect::<Vec<ExpressionInput>>()
            .unwrap();

        func_return.ir(output, context, scope, &mut ret_type)
    }

    pub fn ir_assign(
        identifiers: &Vec<Ident>,
        expressions: &ExpressionList,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
    ) -> Result<(), std::io::Error> {
        let mut expression_inputs = identifiers
            .iter()
            .map(|ident| {
                let variable = match scope.get_entry(ident) {
                    Some(variable) => variable,
                    None => panic!("No variable in scope by name {}", ident),
                };

                match variable {
                    ScopeEntry::Variable(variable) => {
                        let mut store_to = String::new();
                        write!(
                            &mut store_to,
                            "{}* %{}",
                            variable.var_decl.var_type.get_ir_type(),
                            variable.register
                        )
                        .unwrap();

                        ExpressionInput {
                            data_type: variable.var_decl.var_type.clone(),
                            store_to: Some(store_to),
                        }
                    }

                    _ => panic!(
                        "Expected {} to be variable, but it is {:?}",
                        ident, variable
                    ),
                }
            })
            .collect();

        expressions.ir(output, context, scope, &mut expression_inputs)
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
    ) -> Result<usize, std::io::Error> {
        let register = context.claim_register();
        writeln!(
            out,
            "  %{} = alloca {}",
            register,
            self.var_type.get_ir_type()
        )?;

        scope.set_entry(ScopeEntry::Variable(ScopeVariable {
            var_decl: self.clone(),
            register,
        }));

        Ok(register)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I32,
    Any,
}

impl Type {
    pub fn get_ir_type(&self) -> &'static str {
        match self {
            Type::I32 => "i32",
            Type::Any => panic!("Attempted to get llvm ir type of Any!"),
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
