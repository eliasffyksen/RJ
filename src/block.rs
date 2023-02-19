use std::fmt::Debug;

use crate::scope::Scopable;
use crate::stmt::Stmt;
use crate::Rule;
use crate::symbol_ref::SymbolError;

#[derive(Debug, Default)]
pub struct Block {
    statements: Vec<Stmt>,
}

impl Block {
    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &(impl Scopable + std::fmt::Debug),
    ) -> Result<(), SymbolError> {
        let mut scope = scope.new_scope();

        for statement in &self.statements {
            statement.ir(output, context, &mut scope)?
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
