use std::fmt::{self, Write as _};
use std::slice::IterMut;

use pest::iterators::Pair;

use crate::block::Block;
use crate::expression::{ExpressionInput, ExpressionList};
use crate::ident::{Ident, IdentImpl};
use crate::scope::{Scopable, ScopeEntry, ScopeFunction};
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
    input_expressions: ExpressionList,
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
            input_expressions: expressions.unwrap_or(Default::default()),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
        return_data: &mut IterMut<ExpressionInput>,
    ) -> Result<(), std::io::Error> {
        let function = self.get_function_from_scope(scope);

        let function_name = function.name.clone();
        let (return_variables, temporary_variables) =
            Self::generate_temporary_variables(output, function, return_data, context);

        let mut llvm_call_args = vec![];

        let mut function_inputs = self.generate_function_inputs(function);

        self.input_expressions
            .ir(output, context, scope, &mut function_inputs).unwrap();

        for variable in return_variables {
            llvm_call_args.push(variable);
        }

        for input in function_inputs {
            let store_to = input.store_to.unwrap();
            llvm_call_args.push(store_to);
        }

        writeln!(output, "  call void @{}({})", function_name, llvm_call_args.join(", "))?;

        Self::move_temporary_registers(output, temporary_variables, context);

        Ok(())
    }

    fn get_function_from_scope<'a>(
        &self,
        scope: &'a mut impl Scopable,
    ) -> &'a ScopeFunction {
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

        if self.input_expressions.expressions.len() != function.args.len() {
            panic!(
                "Function {} takes {} arguments, {} were given",
                function.name,
                function.args.len(),
                self.input_expressions.expressions.len(),
            )
        }

        function
    }

    fn generate_function_inputs(
        &self,
        function: &ScopeFunction,
    ) -> Vec<ExpressionInput> {
        function
            .args
            .iter()
            .map(|t| ExpressionInput {
                data_type: t.clone(),
                store_to: None,
            })
            .collect()
    }

    fn generate_temporary_variables<'a>(
        output: &mut impl std::io::Write,
        function: &ScopeFunction,
        expression_inputs: &'a mut IterMut<ExpressionInput>,
        context: &mut crate::IRContext,
    ) -> (Vec<String>, Vec<(usize, &'a mut ExpressionInput)>) {
        let mut post_call_moves = vec![];
        let mut return_variables = vec![];

        for return_type in &function.returns {
            let input = expression_inputs
                .next()
                .expect("function returns to many values");

            if input.data_type != *return_type {
                panic!(
                    "Incompatible return type, expected {:?} got {:?}",
                    return_type, input.data_type
                );
            }

            if let Some(store_to) = &input.store_to {
                return_variables.push(store_to.clone());

                continue;
            }

            let (temporary_variable, store_to) =
                Self::create_temporary_register(output, context, return_type);

            post_call_moves.push((temporary_variable, input));
            return_variables.push(store_to);
        }

        (return_variables, post_call_moves)
    }

    fn create_temporary_register(
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        data_type: &Type,
    ) -> (usize, String) {
        let temporary_variable = context.claim_register();

        writeln!(
            output,
            "  %{} = alloca {}",
            temporary_variable,
            data_type.get_ir_type()
        )
        .unwrap();

        let mut store_to = String::new();
        write!(
            &mut store_to,
            "{}* %{}",
            data_type.get_ir_type(),
            temporary_variable
        )
        .unwrap();

        (temporary_variable, store_to)
    }

    fn move_temporary_registers(
        output: &mut impl std::io::Write,
        temporary_variables: Vec<(usize, &mut ExpressionInput)>,
        context: &mut crate::IRContext,
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
    }
}
