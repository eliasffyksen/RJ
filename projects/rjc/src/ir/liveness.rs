//! Liveness: which locals hold a value that may still be read later.

use std::collections::HashSet;

use super::*;

/// `live_after[block][i]` holds the locals live right after statement `i`
/// of `block` (before the terminator, for the last statement).
pub struct Liveness {
    pub live_after: Vec<Vec<HashSet<LocalId>>>,
}

fn operand_uses(operand: &Operand, uses: &mut Vec<LocalId>) {
    if let Operand::Local(id) = operand {
        uses.push(*id);
    }
}

/// The locals a statement reads and the locals it overwrites completely.
fn uses_and_defs(stmt: &Stmt) -> (Vec<LocalId>, Vec<LocalId>) {
    let mut uses = vec![];

    match stmt {
        Stmt::Assign { dest, value } => {
            match value {
                Rvalue::Use(operand) | Rvalue::Repeat(operand, _) => {
                    operand_uses(operand, &mut uses)
                }
                Rvalue::Binary(_, lhs, rhs) => {
                    operand_uses(lhs, &mut uses);
                    operand_uses(rhs, &mut uses);
                }
                Rvalue::Array(elems) => elems.iter().for_each(|e| operand_uses(e, &mut uses)),
                Rvalue::Index(array, index) => {
                    uses.push(*array);
                    operand_uses(index, &mut uses);
                }
            }
            (uses, vec![*dest])
        }

        // Updates one element, so the rest of the old value is still used.
        Stmt::SetIndex {
            array,
            index,
            value,
        } => {
            uses.push(*array);
            operand_uses(index, &mut uses);
            operand_uses(value, &mut uses);
            (uses, vec![])
        }

        Stmt::Call { args, dests, .. } => {
            args.iter().for_each(|arg| operand_uses(arg, &mut uses));
            (uses, dests.clone())
        }
    }
}

fn terminator_uses(function: &Function, terminator: &Terminator) -> Vec<LocalId> {
    match terminator {
        Terminator::Goto(_) => vec![],
        Terminator::Branch { cond, .. } => {
            let mut uses = vec![];
            operand_uses(cond, &mut uses);
            uses
        }
        // The caller reads the return slots.
        Terminator::Return => function.returns.clone(),
    }
}

/// Live set before a block, given the set live after its terminator.
fn transfer(function: &Function, block: &BasicBlock, live: &mut HashSet<LocalId>) {
    live.extend(terminator_uses(function, &block.terminator));

    for stmt in block.stmts.iter().rev() {
        let (uses, defs) = uses_and_defs(stmt);

        for def in defs {
            live.remove(&def);
        }
        live.extend(uses);
    }
}

pub fn liveness(function: &Function) -> Liveness {
    let count = function.blocks.len();
    let mut live_in = vec![HashSet::new(); count];

    // Iterate to a fixpoint; loops need more than one pass.
    let mut changed = true;
    while changed {
        changed = false;

        for (i, block) in function.blocks.iter().enumerate().rev() {
            let mut live = HashSet::new();
            for succ in block.terminator.successors() {
                live.extend(live_in[succ.0].iter().copied());
            }

            transfer(function, block, &mut live);

            if live != live_in[i] {
                live_in[i] = live;
                changed = true;
            }
        }
    }

    let live_after = function
        .blocks
        .iter()
        .map(|block| {
            let mut live = HashSet::new();
            for succ in block.terminator.successors() {
                live.extend(live_in[succ.0].iter().copied());
            }
            live.extend(terminator_uses(function, &block.terminator));

            let mut after = vec![HashSet::new(); block.stmts.len()];
            for (i, stmt) in block.stmts.iter().enumerate().rev() {
                after[i] = live.clone();

                let (uses, defs) = uses_and_defs(stmt);
                for def in defs {
                    live.remove(&def);
                }
                live.extend(uses);
            }
            after
        })
        .collect();

    Liveness { live_after }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, syntax};

    fn main_of(source: &str) -> Function {
        let mut program = lower(&check::check(&syntax::parse(source).unwrap()).unwrap());
        let main = program.main.0;
        program.functions.swap_remove(main)
    }

    fn local_named(function: &Function, name: &str) -> LocalId {
        let index = function
            .locals
            .iter()
            .position(|l| l.kind == LocalKind::Var(name.to_string()))
            .unwrap();
        LocalId(index)
    }

    /// Whether `name` is live after the first call in `main`.
    fn live_after_first_call(source: &str, name: &str) -> bool {
        let main = main_of(source);
        let x = local_named(&main, name);
        let liveness = liveness(&main);

        for (b, block) in main.blocks.iter().enumerate() {
            for (i, stmt) in block.stmts.iter().enumerate() {
                if matches!(stmt, Stmt::Call { .. }) {
                    return liveness.live_after[b][i].contains(&x);
                }
            }
        }
        panic!("no call");
    }

    const F: &str = "fn f(a: [i32; 2]): i32 { return a[0] }";

    #[test]
    fn dead_after_last_use() {
        let source = format!(
            "{F} fn main(): i32 {{ x: [i32; 2] x = [1, 2] return f(x) }}"
        );
        assert!(!live_after_first_call(&source, "x"));
    }

    #[test]
    fn live_when_used_later() {
        let source = format!(
            "{F} fn main(): i32 {{ x: [i32; 2] x = [1, 2] y: i32 y = f(x) return x[1] + y }}"
        );
        assert!(live_after_first_call(&source, "x"));
    }

    #[test]
    fn live_around_a_loop() {
        let source = format!(
            "{F} fn main(): i32 {{
               x: [i32; 2]
               y: i32
               x, y = [1, 2], 0
               while y < 3 {{ y = y + f(x) }}
               return y
             }}"
        );
        // The next iteration reads `x` again.
        assert!(live_after_first_call(&source, "x"));
    }

    #[test]
    fn dead_when_overwritten_first() {
        let source = format!(
            "{F} fn main(): i32 {{
               x: [i32; 2]
               y: i32
               x = [1, 2]
               y = f(x)
               x = [3, 4]
               return x[0] + y
             }}"
        );
        assert!(!live_after_first_call(&source, "x"));
    }
}
