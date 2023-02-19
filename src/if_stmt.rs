use std::fmt::Debug;

use pest::iterators::Pair;

use crate::{
    block::Block,
    check_rule,
    expression::{Expression, ExpressionInput},
    scope::Scopable,
    ast_type::Type,
    symbol_ref::{SymbolRef, SymbolError},
    unexpected_pair, Rule,
};

#[derive(Debug)]
pub struct If {
    symbol: SymbolRef,
    expression: Expression,
    if_block: Block,
}

impl If {
    pub fn ast(pair: Pair<Rule>) -> Self {
        check_rule(&pair, Rule::if_stmt);

        let symbol = SymbolRef::from_pair(&pair);

        let mut expression = None;
        let mut if_block = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::expr_elm => expression = Some(Expression::ast(pair)),
                Rule::block => if_block = Some(Block::ast(pair)),

                _ => unexpected_pair(&pair),
            }
        }

        If {
            symbol,
            expression: expression.unwrap(),
            if_block: if_block.unwrap(),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
    ) -> Result<bool, SymbolError> {
        let mut condition_input = vec![ExpressionInput {
            data_type: Type::Bool,
            store_to: None,
        }];

        self.expression
            .ir(output, context, scope, &mut condition_input.iter_mut())?;

        let condition_input = condition_input.pop().unwrap();

        let label_if = context.claim_register();
        let mut block_if_output = vec![];

        let if_returned = self.if_block.ir(&mut block_if_output, context, scope)?;

        let label_done = context.claim_register();

        writeln!(output, "  br {}, label %{}, label %{}", condition_input.store_to.unwrap(), label_if, label_done).unwrap();
        writeln!(output).unwrap();

        writeln!(output, "{}:", label_if).unwrap();
        output.write(&block_if_output).unwrap();
        if !if_returned {
            writeln!(output, "  br label %{}", label_done).unwrap();
        }

        writeln!(output).unwrap();
        writeln!(output, "{}:", label_done).unwrap();

        Ok(false)
    }
}
