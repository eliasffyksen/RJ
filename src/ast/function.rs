use std::io;

use crate::ast;
use crate::ast::scope;
use crate::ast::scope::Scopable;
use crate::ast::stmt;
use crate::parser;

#[derive(Debug, Default)]
pub struct Function {
    pub symbol: ast::SymbolRef,
    pub name: Option<ast::Ident>,
    pub block: stmt::Block,
    pub args: Vec<stmt::VarDecl>,
    pub ret_type: Vec<ast::Type>,
}

impl Function {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        assert!(pair.as_rule() == parser::Rule::func);

        let mut function: Function = Default::default();
        function.symbol = ast::SymbolRef::from_pair(&pair);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::ident => function.name = Some(ast::Ident::ast(pair)),
                parser::Rule::arg_def => function.ast_args(pair),
                parser::Rule::block => function.block = stmt::Block::ast(pair),
                parser::Rule::ret_type => function.ast_ret_type(pair),

                _ => unexpected_pair!(pair),
            }
        }

        function
    }

    fn ast_args(&mut self, pair: parser::Pair<parser::Rule>) {
        assert!(pair.as_rule() == parser::Rule::arg_def);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::var_decl => self.args.push(stmt::VarDecl::ast(pair)),

                _ => unexpected_pair!(pair),
            }
        }
    }

    fn ast_ret_type(&mut self, pair: parser::Pair<parser::Rule>) {
        assert!(pair.as_rule() == parser::Rule::ret_type);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::var_type => self.ret_type.push(ast::Type::ast(pair)),

                _ => unexpected_pair!(&pair),
            }
        }
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut ast::IRContext,
        scope: &impl scope::Scopable,
    ) -> Result<(), ast::SymbolError> {
        let name = match &self.name {
            Some(name) => name,
            _ => panic!(
                "Can not write LLVM IR for function without name: {:?}",
                self
            ),
        };

        let mut scope = scope.new_scope();

        scope.set_ret_type(self.ret_type.clone());

        write!(output, "define void @{}", name.get()).unwrap();

        self.ir_args(output, context, &mut scope).unwrap();

        let has_returned = self.block.ir(output, context, &mut scope)?;
        if !has_returned {
            if self.ret_type.len() != 0 {
                return Err(ast::SymbolError {
                    error: Box::new("No return value".to_string()),
                    symbol: self.symbol.clone(),
                });
            }

            writeln!(output, "  ret void").unwrap();
        }

        writeln!(output, "}}").unwrap();

        Ok(())
    }

    fn ir_args(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
    ) -> Result<(), io::Error> {
        write!(output, "(")?;

        context.clear_register();

        self.ir_ret_type(output, context)?;

        let mut arguments = vec![];

        for arg in &self.args {
            let register = context.claim_register();

            if register != 0 {
                write!(output, ", ")?;
            }

            arguments.push((arg.clone(), register));

            write!(output, "{} %{}", arg.var_type.get_ir_type(), register)?;
        }

        writeln!(output, ") {{")?;

        context.claim_register();

        for (arg, input_register) in arguments {
            let output_register = arg.ir(output, context, scope)?;

            writeln!(
                output,
                "  store {} %{}, {}* %{}",
                arg.var_type.get_ir_type(),
                input_register,
                arg.var_type.get_ir_type(),
                output_register,
            )?;
        }

        Ok(())
    }

    fn ir_ret_type(
        &self,
        output: &mut impl std::io::Write,
        context: &mut ast::IRContext,
    ) -> Result<(), io::Error> {
        for var_type in &self.ret_type {
            let register = context.claim_register();

            if register != 0 {
                write!(output, ", ")?;
            }

            write!(output, "{}* %{}", var_type.get_ir_type(), register)?;
        }

        Ok(())
    }
}
