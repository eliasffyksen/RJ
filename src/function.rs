use pest::iterators::Pair;

use crate::block::Block;
use crate::scope::{NonScope, Scopable, ScopeEntry};
use crate::stmt::{VarDecl, Type};
use crate::{Rule, IRContext, check_rule, unexpected_pair};
use crate::ident::{Ident, IdentImpl};

#[derive(Debug, Default)]
pub struct Function {
    pub name: Option<Ident>,
    pub block: Block,
    args: Vec<VarDecl>,
    ret_type: Vec<Type>,
}

impl Function {
    pub fn ast(pair: Pair<Rule>) -> Function {
        check_rule(&pair, Rule::func);

        let mut function: Function = Default::default();

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => function.name = Some(Ident::ast(pair)),
                Rule::arg_def => function.ast_args(pair),
                Rule::block => function.block = Block::ast(pair),
                Rule::ret_type => function.ast_ret_type(pair),

                _ => panic!("Invalid pair in function: {:?}", pair)
            }
        }

        function
    }

    fn ast_args(&mut self, pair: Pair<Rule>) {
        check_rule(&pair, Rule::arg_def);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::var_decl => self.args.push(VarDecl::ast(pair)),

                _ => unexpected_pair(&pair)
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

    pub fn ir(&self, output: &mut impl std::io::Write, context: &mut IRContext) -> Result<(), std::io::Error> {
        let name = match &self.name {
            Some(name) => name,
            _ => panic!(
                "Can not write LLVM IR for function without name: {:?}",
                self
            ),
        };

        let mut scope = NonScope{}.new_scope();

        write!(output, "define void @{}", name)?;

        self.ir_args(output, context, &mut scope)?;

        writeln!(output, " {{")?;

        context.claim_register();

        self.block.ir(output, context, &mut scope)?;

        writeln!(output, "  ret void")?;
        writeln!(output, "}}")?;

        Ok(())
    }

    fn ir_args(
        &self, output: &mut impl std::io::Write,
        context: &mut IRContext,
        scope: &mut impl Scopable
    )-> Result<(), std::io::Error> {

        write!(output, "(")?;

        context.clear_register();

        self.ir_ret_type(output, context)?;

        for arg in &self.args {
            let register = context.claim_register();

            if register != 0 {
                write!(output, ", ")?;
            }

            scope.set_entry(ScopeEntry{
                var_decl: arg.clone(),
                register
            });

            write!(output, "{} %{}", arg.var_type.get_ir_type(), register)?;
        }

        write!(output, ")")?;

        Ok(())
    }

    fn ir_ret_type(
        &self, output: &mut impl std::io::Write,
        context: &mut IRContext,
    )-> Result<(), std::io::Error> {

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
