use std::collections::VecDeque;
use std::fmt::Write;
use std::io;
use std::vec;

use crate::ast;
use crate::ast::expr;
use crate::ast::scope;
use crate::ast::stmt;
use crate::parser;

#[derive(Debug)]
pub enum Stmt {
    VarDecl(VarDecl),
    FuncReturn(expr::List),
    Assign((Vec<ast::Ident>, expr::List)),
    FuncCall(expr::FuncCall),
    If(stmt::If),
}

impl Stmt {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Stmt {
        assert!(pair.as_rule() == parser::Rule::stmt);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::var_decl => return Self::VarDecl(VarDecl::ast(pair)),
                parser::Rule::func_ret => return Self::ast_return(pair),
                parser::Rule::assign => return Self::ast_assign(pair),
                parser::Rule::func_call => return Self::FuncCall(expr::FuncCall::ast(pair)),
                parser::Rule::if_stmt => return Self::If(stmt::If::ast(pair)),

                _ => panic!("Unexpected pair: {:?}", pair),
            }
        }

        panic!("No pairs in statement");
    }

    fn ast_return(pair: parser::Pair<parser::Rule>) -> Self {
        assert!(pair.as_rule() == parser::Rule::func_ret);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::expr_list => {
                    return Stmt::FuncReturn(expr::List::ast(pair))
                }

                _ => unexpected_pair!(pair),
            }
        }

        Stmt::FuncReturn(Default::default())
    }

    fn ast_assign(pair: parser::Pair<parser::Rule>) -> Self {
        assert!(pair.as_rule() == parser::Rule::assign);

        let mut identifiers = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::expr_list => {
                    return Self::Assign((identifiers, expr::List::ast(pair)))
                }
                parser::Rule::ident => identifiers.push(ast::Ident::ast(pair)),

                _ => unexpected_pair!(pair),
            }
        }

        panic!("No expression in return statement")
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
    ) -> Result<bool, ast::Error> {
        match self {
            Stmt::VarDecl(var_decl) => {
                var_decl.ir(output, context, scope).unwrap();
                Ok(false)
            }

            Stmt::FuncReturn(func_return) => {
                Self::ir_return(func_return, output, context, scope)?;
                Ok(true)
            }

            Stmt::Assign((identifiers, expressions)) => {
                Self::ir_assign(identifiers, expressions, output, context, scope)?;
                Ok(false)
            }

            Stmt::FuncCall(function_call) => {
                let mut empty = VecDeque::new();
                function_call.ir(output, context, scope, &mut empty)?;
                Ok(false)
            }

            Stmt::If(if_stmt) => if_stmt.ir(output, context, scope),
        }
    }

    pub fn ir_return(
        func_return: &expr::List,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
    ) -> Result<(), ast::Error> {
        let mut ret_type = scope
            .get_ret_type()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut store_to = String::new();
                write!(&mut store_to, "{}* %{}", t.get_ir_type(), i)?;

                let result: Result<_, std::fmt::Error> = Ok(expr::Req {
                    data_type: t.clone(),
                    store_to: Some(store_to),
                });

                result
            })
            .try_collect::<VecDeque<expr::Req>>()
            .unwrap();

        func_return.ir(output, context, scope, &mut ret_type)?;

        writeln!(output, "  ret void").unwrap();

        Ok(())
    }

    pub fn ir_assign(
        identifiers: &Vec<ast::Ident>,
        expressions: &expr::List,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
    ) -> Result<(), ast::Error> {
        let mut expression_inputs = identifiers
            .iter()
            .map(|ident| {
                let variable = match scope.get_entry(ident) {
                    Some(variable) => variable,
                    None => panic!("No variable in scope by name {}", ident.get()),
                };

                match variable {
                    scope::Entry::Variable(variable) => {
                        let mut store_to = String::new();
                        write!(
                            &mut store_to,
                            "{}* %{}",
                            variable.var_decl.var_type.get_ir_type(),
                            variable.register
                        )
                        .unwrap();

                        expr::Req {
                            data_type: variable.var_decl.var_type.clone(),
                            store_to: Some(store_to),
                        }
                    }

                    _ => panic!(
                        "Expected {} to be variable, but it is {:?}",
                        ident.get(),
                        variable
                    ),
                }
            })
            .collect();

        expressions.ir(output, context, scope, &mut expression_inputs)
    }
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub ident: ast::Ident,
    pub var_type: ast::Type,
}

impl VarDecl {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> VarDecl {
        if pair.as_rule() != parser::Rule::var_decl {
            panic!(
                "Attempted to generate VarDecl from non VarDecl pair: {:?}",
                pair
            )
        }

        let mut ident = None;
        let mut var_type = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::ident => ident = Some(ast::Ident::ast(pair)),
                parser::Rule::var_type => var_type = Some(ast::Type::ast(pair)),

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
        out: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
    ) -> Result<usize, std::io::Error> {
        let register = context.claim_register();
        writeln!(
            out,
            "  %{} = alloca {}",
            register,
            self.var_type.get_ir_type()
        )?;

        scope.set_entry(scope::Entry::Variable(scope::Variable {
            var_decl: self.clone(),
            register,
        }));

        Ok(register)
    }
}
