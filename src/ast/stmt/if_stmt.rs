use std::io;

use crate::ast;
use crate::ast::{expr, scope, stmt};
use crate::parser;

#[derive(Debug)]
pub struct If {
    symbol: ast::Symbol,
    expression: expr::Expr,
    if_block: stmt::Block,
}

impl If {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        assert!(pair.as_rule() == parser::Rule::if_stmt);

        let symbol = ast::Symbol::from_pair(&pair);

        let mut expression = None;
        let mut if_block = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::expr_elm => expression = Some(expr::Expr::ast(pair)),
                parser::Rule::block => if_block = Some(stmt::Block::ast(pair)),

                _ => unexpected_pair!(pair),
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
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
    ) -> Result<bool, ast::Error> {
        let mut condition_input = vec![expr::Input {
            data_type: ast::Type::Bool,
            store_to: None,
        }];

        self.expression
            .ir(output, context, scope, &mut condition_input.iter_mut())?;

        let condition_input = condition_input.pop().unwrap();

        let label_if = context.claim_register();
        let mut block_if_output = vec![];

        let if_returned = self.if_block.ir(&mut block_if_output, context, scope)?;

        let label_done = context.claim_register();

        writeln!(
            output,
            "  br {}, label %{}, label %{}",
            condition_input.store_to.unwrap(),
            label_if,
            label_done
        )
        .unwrap();
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
