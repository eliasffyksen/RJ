use std::{io, fs};

use pest::{iterators::Pair, Parser as _};
use rjc_ast::{Pool, PoolRef, PoolType, Function, Module};

use crate::{Rule, ASTParser, Parser};

pub fn from_file(path: String) -> io::Result<Pool> {
    let input = fs::read_to_string(&path)?;

    let mut pool = Pool::new(path, input.clone());

    let pair = Parser::parse(Rule::module, input.as_str())
        .unwrap()
        .next()
        .unwrap();

    let _ = Module::parse(&mut pool, pair);

    Ok(pool)
}

impl ASTParser for Module {
    fn parse(pool: &mut Pool, pair: Pair<Rule>) -> PoolRef<Self>
    where
        Self: PoolType + Sized,
    {
        let inner = match pair.as_rule() {
            Rule::module => pair.into_inner(),

            _ => unexpected_pair!(pair),
        };

        let mut functions = vec![];

        for pair in inner {
            match pair.as_rule() {
                Rule::func => {
                    let function = Function::parse(pool, pair);
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
