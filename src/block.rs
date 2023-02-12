use crate::GenerateAST;
use crate::GenerateIR;
use crate::Rule;
use crate::stmt::Stmt;

#[derive(Debug, Default)]
pub struct Block {
    statements: Vec<Stmt>
}

impl GenerateAST<Block> for Block {
    fn generate_ast(pair: pest::iterators::Pair<crate::Rule>) -> Block {
        if pair.as_rule() != Rule::block {
            panic!("Attempted generating block from non block pair: {:?}", pair)
        }

        let mut block: Block = Default::default();


        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::stmt => block.statements.push(Stmt::generate_ast(pair)),

                _ => panic!("Unexpected pair: {:?}", pair),
            }
        }

        block
    }
}

impl GenerateIR for Block {
    fn generate_ir(&self, out: &mut impl std::io::Write, context: &mut crate::IRContext) -> Result<(), std::io::Error> {
        for statement in &self.statements {
            statement.generate_ir(out, context)?
        }

        Ok(())
    }
}
