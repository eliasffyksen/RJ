use std::{fs, io};

use super::*;
use crate::ast as ast;

pub fn from_file(path: String) -> io::Result<ast::Pool> {
    let input = fs::read_to_string(&path)?;

    let mut pool = ast::Pool::new(path, input.clone());

    let pair = Parser::parse(Rule::module, input.as_str())
        .unwrap()
        .next()
        .unwrap();

    let _ = ast::Module::parse(&mut pool, pair);

    Ok(pool)
}

impl ASTParser for ast::Module {
    fn parse(pool: &mut ast::Pool, pair: Pair<Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType + Sized,
    {
        let inner = match pair.as_rule() {
            Rule::module => pair.into_inner(),

            _ => unexpected_pair!(pair),
        };

        let mut functions = vec![];

        for pair in inner {
            match pair.as_rule() {
                Rule::func => {
                    let function = ast::Function::parse(pool, pair);
                    functions.push(function);
                },
                Rule::EOI => break,

                _ => unexpected_pair!(pair),
            }
        }

        pool.add(Self {
            id: pool.len(),
            functions,
        })
    }
}
