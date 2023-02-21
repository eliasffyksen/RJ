use std::collections::VecDeque;
use std::io;

use crate::ast;
use crate::ast::expr;
use crate::ast::scope;
use crate::parser;

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
        return_data: &mut VecDeque<expr::Req>,
    ) -> Result<(), ast::Error> {
        let function = self.get_function_from_scope(scope);

        let function_name = function.name.clone();
        let (return_variables, temporary_variables) =
            Self::generate_temporary_variables(output, function, return_data, context);

        let mut function_inputs = self.generate_function_inputs(function);

        self.input_expressions
            .ir(output, context, scope, &mut function_inputs)?;

        let mut llvm_call_args = vec![];

        for variable in return_variables {
            llvm_call_args.push(variable);
        }

        for input in function_inputs {
            let store_to = input.store_to.unwrap();
            llvm_call_args.push(store_to);
        }

        writeln!(
            output,
            "  call void @{}({})",
            function_name.get(),
            llvm_call_args.join(", ")
        )
        .unwrap();

        Self::move_temporary_registers(output, temporary_variables, context);

        Ok(())
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

    fn generate_temporary_variables(
        output: &mut impl io::Write,
        function: &scope::Function,
        requests: &mut VecDeque<expr::Req>,
        context: &mut ast::IRContext,
    ) -> (Vec<String>, Vec<(usize, expr::Req)>) {
        let mut post_call_moves = vec![];
        let mut return_variables = vec![];

        for return_type in &function.returns {
            let request = requests.pop_front()
                .expect("function returns to many values");

            if request.data_type != *return_type {
                panic!(
                    "Incompatible return type, expected {:?} got {:?}",
                    return_type, request.data_type
                );
            }

            if let Some(store_to) = &request.store_to {
                return_variables.push(store_to.clone());

                continue;
            }

            let (temporary_variable, store_to) =
                Self::create_temporary_register(output, context, return_type);

            post_call_moves.push((temporary_variable, request));
            return_variables.push(store_to);
        }

        (return_variables, post_call_moves)
    }

    fn create_temporary_register(
        output: &mut impl std::io::Write,
        context: &mut ast::IRContext,
        data_type: &ast::Type,
    ) -> (usize, String) {
        let temporary_variable = context.claim_register();

        writeln!(
            output,
            "  %{} = alloca {}",
            temporary_variable,
            data_type.get_ir_type()
        )
        .unwrap();

        let store_to = format!("{}* %{}", data_type.get_ir_type(), temporary_variable);

        (temporary_variable, store_to)
    }

    fn move_temporary_registers(
        output: &mut impl io::Write,
        temporary_variables: Vec<(usize, expr::Req)>,
        context: &mut ast::IRContext,
    ) {
        for (temporary_register, expression_input) in temporary_variables {
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

            let store_to = format!(
                "{} %{}",
                expression_input.data_type.get_ir_type(),
                output_register
            );

            expression_input.store_to = Some(store_to);
        }
    }
}
