use pest::iterators::Pair;

use rjc_ast::{statement::Statement, Block, AST, ASTRef, ASTType, Symbol};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for Block {
    fn parse(pool: &mut AST, pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        assert!(pair.as_rule() == Rule::block);

        let symbol = Symbol::from_pair(&pair);
        let mut statements = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::stmt => statements.push(Statement::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let block = Block {
            id: pool.len(),
            symbol: symbol,
            statements,
        };

        pool.add(block)
    }
}
