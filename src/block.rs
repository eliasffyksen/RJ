use crate::Rule;
use crate::stmt::Stmt;

#[derive(Debug, Default)]
pub struct Block {
    statements: Vec<Stmt>
}

impl Block {
    pub fn ir(&self, out: &mut impl std::io::Write, context: &mut crate::IRContext) -> Result<(), std::io::Error> {
        for statement in &self.statements {
            statement.ir(out, context)?
        }

        Ok(())
    }

    pub fn ast(pair: pest::iterators::Pair<crate::Rule>) -> Block {
        if pair.as_rule() != Rule::block {
            panic!("Attempted generating block from non block pair: {:?}", pair)
        }

        let mut block: Block = Default::default();


        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::stmt => block.statements.push(Stmt::ast(pair)),

                _ => panic!("Unexpected pair: {:?}", pair),
            }
        }

        block
    }
}
