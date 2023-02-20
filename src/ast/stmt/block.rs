use std::io;

use crate::ast;
use crate::ast::scope;
use crate::ast::stmt;
use crate::parser;

#[derive(Debug, Default)]
pub struct Block {
    statements: Vec<stmt::Stmt>,
}

impl Block {
    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &impl scope::Scopable,
    ) -> Result<bool, ast::SymbolError> {
        let mut scope = scope.new_scope();

        for statement in &self.statements {
            let has_returned = statement.ir(output, context, &mut scope)?;
            if has_returned {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn ast(pair: parser::Pair<parser::Rule>) -> stmt::Block {
        assert!(pair.as_rule() == parser::Rule::block);

        let mut block: Block = Default::default();

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::stmt => block.statements.push(stmt::Stmt::ast(pair)),

                _ => unexpected_pair!(pair),
            }
        }

        block
    }
}
