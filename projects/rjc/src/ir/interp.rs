//! A tiny IR interpreter, used by tests to check that lowering preserves
//! the meaning of programs independently of any backend.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Int(i32),
    Bool(bool),
}

/// Runs `main` and returns its exit code.
pub fn run(program: &Program) -> i32 {
    match call(program, program.main, vec![])[..] {
        [Value::Int(code)] => code,
        ref other => panic!("main returned {other:?}"),
    }
}

pub fn call(program: &Program, func: FuncId, args: Vec<Value>) -> Vec<Value> {
    let function = program.function(func);
    let mut locals: Vec<Option<Value>> = vec![None; function.locals.len()];

    assert_eq!(args.len(), function.params.len());
    for (param, arg) in function.params.iter().zip(args) {
        locals[param.0] = Some(arg);
    }

    let read = |locals: &[Option<Value>], operand: &Operand| match operand {
        Operand::Local(id) => locals[id.0]
            .unwrap_or_else(|| panic!("{}: read of unassigned local {id}", function.name)),
        Operand::Int(value) => Value::Int(*value),
        Operand::Bool(value) => Value::Bool(*value),
    };

    let mut block = BlockId(0);

    loop {
        let data = function.block(block);

        for stmt in &data.stmts {
            match stmt {
                Stmt::Assign { dest, value } => {
                    let value = match value {
                        Rvalue::Use(operand) => read(&locals, operand),
                        Rvalue::Binary(op, lhs, rhs) => {
                            binary(*op, read(&locals, lhs), read(&locals, rhs))
                        }
                    };
                    locals[dest.0] = Some(value);
                }

                Stmt::Call { func, args, dests } => {
                    let args = args.iter().map(|arg| read(&locals, arg)).collect();
                    let results = call(program, *func, args);

                    assert_eq!(results.len(), dests.len());
                    for (dest, result) in dests.iter().zip(results) {
                        locals[dest.0] = Some(result);
                    }
                }
            }
        }

        match &data.terminator {
            Terminator::Goto(target) => block = *target,

            Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                block = match read(&locals, cond) {
                    Value::Bool(true) => *then_block,
                    Value::Bool(false) => *else_block,
                    other => panic!("branch on {other:?}"),
                }
            }

            Terminator::Return => {
                return function
                    .returns
                    .iter()
                    .map(|slot| read(&locals, &Operand::Local(*slot)))
                    .collect();
            }
        }
    }
}

fn binary(op: BinOp, lhs: Value, rhs: Value) -> Value {
    use Value::{Bool, Int};

    match (op, lhs, rhs) {
        (BinOp::Add, Int(l), Int(r)) => Int(l.wrapping_add(r)),
        (BinOp::Sub, Int(l), Int(r)) => Int(l.wrapping_sub(r)),
        (BinOp::Lt, Int(l), Int(r)) => Bool(l < r),
        (BinOp::Le, Int(l), Int(r)) => Bool(l <= r),
        (BinOp::Gt, Int(l), Int(r)) => Bool(l > r),
        (BinOp::Ge, Int(l), Int(r)) => Bool(l >= r),
        (BinOp::Eq, l, r) => Bool(l == r),
        (BinOp::Ne, l, r) => Bool(l != r),
        (op, l, r) => panic!("invalid operands for {op:?}: {l:?}, {r:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, syntax};

    fn lower_source(source: &str) -> Program {
        lower(&check::check(&syntax::parse(source).unwrap()).unwrap())
    }

    #[test]
    fn all_test_programs_exit_with_zero() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test");

        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let program = lower_source(&std::fs::read_to_string(&path).unwrap());

            assert_eq!(run(&program), 0, "{} exited with non-zero", path.display());
        }
    }

    #[test]
    fn parallel_assignment_swaps() {
        let program = lower_source(
            "fn main(): i32 {
               a: i32
               b: i32
               a, b = 1, 2
               a, b = b, a
               return a - b - 1
             }",
        );

        assert_eq!(run(&program), 0);
    }

    #[test]
    fn multiple_return_values_flow_through() {
        let program = lower_source(
            "fn pair(a: i32): i32, i32 { return a, a + 1 }
             fn sub(a: i32, b: i32): i32 { return a - b }
             fn main(): i32 {
               return sub(pair(5)) + 1
             }",
        );

        assert_eq!(run(&program), 0);
    }

    #[test]
    fn overwriting_call_writes_its_argument() {
        // The pattern the in/out optimisation will look for.
        let program = lower_source(
            "fn inc(a: i32): i32 { return a + 1 }
             fn main(): i32 {
               x: i32
               x = 0
               x = inc(x)
               return x - 1
             }",
        );

        let main = program.function(program.main);
        let x = main.params.len(); // first non-param local
        let calls = main
            .blocks
            .iter()
            .flat_map(|block| &block.stmts)
            .filter_map(|stmt| match stmt {
                Stmt::Call { args, dests, .. } => Some((args.clone(), dests.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            calls,
            [(vec![Operand::Local(LocalId(x))], vec![LocalId(x)])]
        );
        assert_eq!(run(&program), 0);
    }

    #[test]
    fn if_with_both_branches_returning_has_no_join_block() {
        let program = lower_source(
            "fn main(): i32 {
               if true { return 0 } else { return 1 }
             }",
        );

        // entry, then, else
        assert_eq!(program.function(program.main).blocks.len(), 3);
    }
}
