use pest::iterators::Pair;

use crate::block::Block;
use crate::stmt::VarDecl;
use crate::{Rule, IRContext, check_rule, unexpected_pair};
use crate::ident::{Ident, IdentImpl};

#[derive(Debug, Default)]
pub struct Function {
    pub name: Option<Ident>,
    pub block: Block,
    pub args: Vec<VarDecl>,
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

    pub fn ir(&self, output: &mut impl std::io::Write, context: &mut IRContext) -> Result<(), std::io::Error> {
        let name = match &self.name {
            Some(name) => name,
            _ => panic!(
                "Can not write LLVM IR for function without name: {:?}",
                self
            ),
        };

        context.clear_register();

        write!(output, "define void @{}", name)?;

        self.ir_args(output, context)?;

        writeln!(output, " {{")?;

        context.claim_register();

        self.block.ir(output, context)?;

        writeln!(output, "  ret void")?;
        writeln!(output, "}}")?;

        Ok(())
    }

    fn ir_args(&self, output: &mut impl std::io::Write, context: &mut IRContext)-> Result<(), std::io::Error> {
        let mut first = true;

        write!(output, "(")?;

        for arg in &self.args {
            if first {
                first = false;
            } else {
                write!(output, ", ")?;
            }

            let register = context.claim_register();

            write!(output, "{} %{}", arg.var_type.get_ir_type(), register)?;
        }

        write!(output, ")")?;

        Ok(())
    }
}
