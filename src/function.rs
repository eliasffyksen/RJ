use pest::iterators::Pair;

use crate::block::Block;
use crate::{GenerateAST, GenerateIR, Rule, IRContext};
use crate::ident::Ident;

#[derive(Debug, Default)]
pub struct Function {
    pub name: Option<Ident>,
    pub block: Block,
}

impl GenerateAST<Function> for Function {
    fn generate_ast(pair: Pair<Rule>) -> Function {
        let mut function: Function = Default::default();

        let inner = match pair.as_rule() {
            Rule::func => pair.into_inner(),
            _ => panic!(
                "Trying to generate function from non function pair: {:?}",
                pair
            ),
        };

        for pair in inner {
            match pair.as_rule() {
                Rule::ident => function.name = Some(Ident::generate_ast(pair)),
                Rule::arg_def => (),
                Rule::block => function.block = Block::generate_ast(pair),

                _ => panic!("Invalid pair in function: {:?}", pair)
            }
        }

        function
    }
}

impl GenerateIR for Function {
    fn generate_ir(&self, out: &mut impl std::io::Write, context: &mut IRContext) -> Result<(), std::io::Error> {
        let name = match &self.name {
            Some(name) => name,
            _ => panic!(
                "Can not write LLVM IR for function without name: {:?}",
                self
            ),
        };

        context.clear_register();

        writeln!(out, "define void @{}() {{", name)?;

        context.claim_register();

        self.block.generate_ir(out, context)?;

        writeln!(out, "  ret void")?;
        writeln!(out, "}}")?;

        Ok(())
    }
}
