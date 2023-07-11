use pest::iterators::Pair;

use rjc_ast::{Block, Function, Ident, AST, ASTRef, ASTType, Symbol, Type, TypeList, Variable};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for Function {
    fn parse(pool: &mut AST, pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        assert!(pair.as_rule() == Rule::func);

        let symbol = Symbol::from_pair(&pair);
        let mut name = None;
        let mut args = vec![];
        let mut block = None;
        let mut return_type = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => name = Some(Ident::parse(pool, pair)),
                Rule::var_decl => args.push(Variable::parse(pool, pair)),
                Rule::block => block = Some(Block::parse(pool, pair)),
                Rule::ret_type => return_type.push(Type::from_str(pair.as_str())),

                _ => unexpected_pair!(pair),
            }
        }

        let function = Function {
            id: pool.len(),
            symbol,
            ident: name.expect("no name defined for function"),
            args,
            block: block.expect("no block defined"),
            return_type: TypeList { list: return_type },
        };

        pool.add(function)
    }
}
