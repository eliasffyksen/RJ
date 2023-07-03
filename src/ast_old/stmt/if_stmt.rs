use std::collections::VecDeque;
use std::io;

use crate::ast;
use crate::ast::{expr, scope, stmt};
use crate::parser;

#[derive(Debug)]
pub struct If {
    symbol: ast::Symbol,
    expression: expr::Expr,
    if_block: stmt::Block,
    else_block: Option<stmt::Block>,
}

impl If {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        assert!(pair.as_rule() == parser::Rule::if_stmt);

        let symbol = ast::Symbol::from_pair(&pair);

        let mut expression = None;
        let mut if_block = None;
        let mut else_block = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::expr_elm => expression = Some(expr::Expr::ast(pair)),
                parser::Rule::block => if_block = Some(stmt::Block::ast(pair)),
                parser::Rule::else_stmt => else_block = Some(Self::ast_else(pair)),

                _ => unexpected_pair!(pair),
            }
        }

        If {
            symbol,
            expression: expression.unwrap(),
            if_block: if_block.unwrap(),
            else_block: else_block,
        }
    }

    fn ast_else(pair: parser::Pair<parser::Rule>) -> stmt::Block {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            parser::Rule::block => stmt::Block::ast(pair),

            _ => unexpected_pair!(pair),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
    ) -> Result<bool, ast::Error> {
        let mut condition_input = VecDeque::new();
        condition_input.push_back(expr::Req {
            data_type: ast::Type::Bool,
            store_to: None,
        });

        let result = self
            .expression
            .ir(output, context, scope, &mut condition_input)?;
        let result = result.into_iter().next().unwrap().unwrap();

        let label_if = context.claim_register();
        let label_done = context.claim_register();
        let _else = self.else_block.as_ref().map(|block| (context.claim_register(), block));
        let label_if_not = _else.map(|(label, _)| label).unwrap_or(label_done);

        writeln!(
            output,
            "  br {}, label %_{}, label %_{}",
            result, label_if, label_if_not
        )
        .unwrap();

        writeln!(output).unwrap();

        writeln!(output, "_{}:", label_if).unwrap();

        let mut all_returned = true;

        let if_returned = self.if_block.ir(output, context, scope)?;

        if !if_returned {
            writeln!(output, "  br label %_{}", label_done).unwrap();
            all_returned = false;
        }

        if let Some((label_else, else_block)) = _else {
            writeln!(output, "_{}:", label_else).unwrap();

            let else_returned = else_block.ir(output, context, scope)?;

            if !else_returned {
                writeln!(output, "  br label %_{}", label_done).unwrap();
                all_returned = false;
            }
        } else {
            all_returned = false;
        }

        if !all_returned {
            writeln!(output).unwrap();
            writeln!(output, "_{}:", label_done).unwrap();
        }

        Ok(all_returned)
    }
}
