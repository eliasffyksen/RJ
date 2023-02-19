use std::fmt::{self, Write as _};
use std::slice::IterMut;

use pest::iterators::Pair;

use crate::block::Block;
use crate::expression::{ExpressionInput, ExpressionList};
use crate::ident::{Ident, IdentImpl};
use crate::scope::{Scopable, ScopeEntry};
use crate::stmt::{Type, VarDecl};
use crate::{check_rule, unexpected_pair, IRContext, Rule};

#[derive(Debug, Default)]
pub struct Function {
    pub name: Option<Ident>,
    pub block: Block,
    pub args: Vec<VarDecl>,
    pub ret_type: Vec<Type>,
}

impl Function {
    pub fn ast(pair: Pair<Rule>) -> Self {
        check_rule(&pair, Rule::func);

        let mut function: Function = Default::default();

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => function.name = Some(Ident::ast(pair)),
                Rule::arg_def => function.ast_args(pair),
                Rule::block => function.block = Block::ast(pair),
                Rule::ret_type => function.ast_ret_type(pair),

                _ => panic!("Invalid pair in function: {:?}", pair),
            }
        }

        function
    }

    fn ast_args(&mut self, pair: Pair<Rule>) {
        check_rule(&pair, Rule::arg_def);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::var_decl => self.args.push(VarDecl::ast(pair)),

                _ => unexpected_pair(&pair),
            }
        }
    }

    fn ast_ret_type(&mut self, pair: Pair<Rule>) {
        check_rule(&pair, Rule::ret_type);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::var_type => self.ret_type.push(Type::ast(pair)),

                _ => unexpected_pair(&pair),
            }
        }
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut IRContext,
        scope: &(impl Scopable + fmt::Debug),
    ) -> Result<(), std::io::Error> {
        let name = match &self.name {
            Some(name) => name,
            _ => panic!(
                "Can not write LLVM IR for function without name: {:?}",
                self
            ),
        };

        let mut scope = scope.new_scope();

        scope.set_ret_type(self.ret_type.clone());

        write!(output, "define void @{}", name)?;

        self.ir_args(output, context, &mut scope)?;

        self.block.ir(output, context, &mut scope)?;

        writeln!(output, "  ret void")?;
        writeln!(output, "}}")?;

        Ok(())
    }

    fn ir_args(
        &self,
        output: &mut impl std::io::Write,
        context: &mut IRContext,
        scope: &mut impl Scopable,
    ) -> Result<(), std::io::Error> {
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
        context: &mut IRContext,
    ) -> Result<(), std::io::Error> {
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

#[derive(Debug)]
pub struct FunctionCall {
    identifier: Ident,
    expressions: ExpressionList,
}

impl FunctionCall {
    pub fn ast(pair: Pair<Rule>) -> Self {
        check_rule(&pair, Rule::func_call);

        let mut identifier = None;
        let mut expressions = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => identifier = Some(Ident::ast(pair)),
                Rule::expr_list => expressions = Some(ExpressionList::ast(pair)),

                _ => unexpected_pair(&pair),
            }
        }

        Self {
            identifier: identifier.expect("No identifier in func call!"),
            expressions: expressions.unwrap_or(Default::default()),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
        expression_inputs: &mut IterMut<ExpressionInput>,
    ) -> Result<(), std::io::Error> {
        let function = scope
            .get_entry(&self.identifier)
            .expect("Function not in scope");
        let function = match function {
            ScopeEntry::Function(x) => x,

            _ => panic!(
                "Expected {} to be function, but is {:?}",
                self.identifier, function
            ),
        };

        if self.expressions.expressions.len() != function.args.len() {
            panic!(
                "Function {} takes {} arguments, {} were given",
                function.name,
                function.args.len(),
                self.expressions.expressions.len(),
            )
        }

        let mut function_inputs = function
            .args
            .iter()
            .map(|t| ExpressionInput {
                data_type: t.clone(),
                store_to: None,
            })
            .collect();

        let function_name = function.name.clone();
        let mut post_call_moves = vec![];
        let return_variables = function
            .returns
            .iter()
            .map(|t| {
                let input = expression_inputs
                    .next()
                    .expect("function returns to many values");

                if input.data_type != *t {
                    panic!(
                        "Incompatible return type, expected {:?} got {:?}",
                        t, input.data_type
                    );
                }

                match &input.store_to {
                    Some(store_to) => store_to.clone(),
                    None => {
                        let temporary_variable = context.claim_register();

                        writeln!(
                            output,
                            "  %{} = alloca {}",
                            temporary_variable,
                            t.get_ir_type()
                        )
                        .unwrap();

                        post_call_moves.push((temporary_variable, input));

                        let mut store_to = String::new();
                        write!(
                            &mut store_to,
                            "{}* %{}",
                            t.get_ir_type(),
                            temporary_variable
                        )
                        .unwrap();

                        store_to
                    }
                }
            })
            .collect::<Vec<_>>();

        self.expressions
            .ir(output, context, scope, &mut function_inputs)?;

        write!(output, "  call void @{}(", function_name)?;

        let mut first = true;

        for store_to in return_variables {
            if !first {
                writeln!(output, ", ")?;
            }
            first = false;

            write!(output, "{}", store_to)?;
        }

        for input in function_inputs {
            let store_to = match input.store_to {
                Some(x) => x,
                None => todo!(),
            };

            if !first {
                write!(output, ", ")?;
            }
            first = false;

            write!(output, "{}", store_to)?;
        }

        writeln!(output, ")")?;

        for (temporary_register, expression_input) in post_call_moves {
            let output_register = context.claim_register();
            writeln!(
                output,
                "  %{} = load {}, {}* %{}",
                output_register,
                expression_input.data_type.get_ir_type(),
                expression_input.data_type.get_ir_type(),
                temporary_register,
            )
            .unwrap();

            let mut store_to = String::new();

            write!(
                &mut store_to,
                "{} %{}",
                expression_input.data_type.get_ir_type(),
                output_register
            )
            .unwrap();

            expression_input.store_to = Some(store_to);
        }

        Ok(())
    }
}
