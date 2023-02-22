use std::collections::VecDeque;
use std::io;

use crate::ast;
use crate::ast::expr;
use crate::ast::scope;
use crate::ast::IRContext;
use crate::parser;

struct ReturnVariable {
    data_type: ast::Type,
    store_to: String,
    temporary: bool,
}

fn return_variables_to_results(
    output: &mut impl io::Write,
    context: &mut IRContext,
    return_variables: Vec<ReturnVariable>,
) -> Vec<Option<expr::Res>> {
    return_variables
        .into_iter()
        .map(|return_variable| {
            if !return_variable.temporary {
                return None;
            }

            let register = context.claim_register();
            writeln!(
                output,
                "  %{} = load {}, {}",
                register, return_variable.data_type, return_variable.store_to,
            ).unwrap();

            Some(expr::Res {
                data_type: return_variable.data_type,
                value: format!("%{}", register),
            })
        })
        .collect()
}

#[derive(Debug)]
pub struct FuncCall {
    identifier: ast::Ident,
    input_expressions: expr::List,
}

impl FuncCall {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        assert!(pair.as_rule() == parser::Rule::func_call);

        let mut identifier = None;
        let mut expressions = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::ident => identifier = Some(ast::Ident::ast(pair)),
                parser::Rule::expr_list => expressions = Some(expr::List::ast(pair)),

                _ => unexpected_pair!(&pair),
            }
        }

        Self {
            identifier: identifier.expect("No identifier in func call!"),
            input_expressions: expressions.unwrap_or(Default::default()),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        return_requests: &mut VecDeque<expr::Req>,
    ) -> Result<Vec<Option<expr::Res>>, ast::Error> {
        let function = self.get_function_from_scope(scope);

        let function_name = function.name.clone();
        let return_variables =
            Self::generate_return_variables(output, function, return_requests, context);

        let mut function_inputs = self.generate_function_inputs(function);

        let mut llvm_call_args: Vec<String> = return_variables
            .iter().map(|r| r.store_to.clone()).collect();

        llvm_call_args.extend(self
            .input_expressions
            .ir(output, context, scope, &mut function_inputs)?
            .into_iter()
            .map(|result| {
                let result = result.unwrap();
                format!("{}", result)
            }));

        writeln!(
            output,
            "  call void @{}({})",
            function_name.get(),
            llvm_call_args.join(", ")
        )
        .unwrap();

        Ok(return_variables_to_results(output, context, return_variables))
    }

    fn get_function_from_scope<'a>(
        &self,
        scope: &'a mut impl scope::Scopable,
    ) -> &'a scope::Function {
        let function = scope
            .get_entry(&self.identifier)
            .expect("Function not in scope");
        let function = match function {
            scope::Entry::Function(x) => x,

            _ => panic!(
                "Expected {} to be function, but is {:?}",
                self.identifier.get(),
                function
            ),
        };

        if self.input_expressions.expressions.len() != function.args.len() {
            panic!(
                "Function {} takes {} arguments, {} were given",
                function.name.get(),
                function.args.len(),
                self.input_expressions.expressions.len(),
            )
        }

        function
    }

    fn generate_function_inputs(&self, function: &scope::Function) -> VecDeque<expr::Req> {
        function
            .args
            .iter()
            .map(|t| expr::Req {
                data_type: t.clone(),
                store_to: None,
            })
            .collect()
    }

    fn generate_return_variables(
        output: &mut impl io::Write,
        function: &scope::Function,
        requests: &mut VecDeque<expr::Req>,
        context: &mut ast::IRContext,
    ) -> Vec<ReturnVariable> {
        let mut result = vec![];

        for return_type in &function.returns {
            let request = requests
                .pop_front()
                .expect("function returns to many values");

            if request.data_type != *return_type {
                panic!(
                    "Incompatible return type, expected {:?} got {:?}",
                    return_type, request.data_type
                );
            }

            if let Some(store_to) = request.store_to {
                result.push(ReturnVariable {
                    data_type: return_type.clone(),
                    store_to,
                    temporary: false,
                });

                continue;
            }

            let temporary_variable = context.claim_register();

            writeln!(output, "  %{} = alloca {}", temporary_variable, return_type,).unwrap();

            result.push(ReturnVariable {
                data_type: return_type.clone(),
                store_to: format!("{}* %{}", return_type, temporary_variable),
                temporary: true,
            });
        }

        result
    }
}
