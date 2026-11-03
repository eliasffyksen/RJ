//! A tiny IR interpreter, used by tests to check that lowering preserves
//! the meaning of programs independently of any backend.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Int(i32),
    Bool(bool),
    Array(Vec<Value>),
    Struct(Vec<Value>),
}

/// An evaluated path step.
enum Step {
    Index(Value),
    Field(usize),
}

/// The part of `value` that `path` leads to.
fn part<'v>(value: &'v mut Value, path: &[Step]) -> &'v mut Value {
    let Some((step, rest)) = path.split_first() else {
        return value;
    };

    let inner = match (step, value) {
        (Step::Index(index), Value::Array(elems)) => {
            let i = array_index(elems, index.clone());
            &mut elems[i]
        }
        (Step::Field(field), Value::Struct(fields)) => &mut fields[*field],
        (_, value) => panic!("path does not match value {value:?}"),
    };

    part(inner, rest)
}

fn array_index(array: &[Value], index: Value) -> usize {
    match index {
        Value::Int(i) if i >= 0 && (i as usize) < array.len() => i as usize,
        Value::Int(i) => panic!("index out of bounds: {i} for length {}", array.len()),
        other => panic!("array index {other:?}"),
    }
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
            .clone()
            .unwrap_or_else(|| panic!("{}: read of unassigned local {id}", function.name)),
        Operand::Int(value) => Value::Int(*value),
        Operand::Bool(value) => Value::Bool(*value),
    };

    let eval_path = |locals: &[Option<Value>], path: &[Elem]| {
        path.iter()
            .map(|elem| match elem {
                Elem::Index(index) => Step::Index(read(locals, index)),
                Elem::Field(field) => Step::Field(*field),
            })
            .collect::<Vec<_>>()
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
                        Rvalue::Array(elems) => {
                            Value::Array(elems.iter().map(|e| read(&locals, e)).collect())
                        }
                        Rvalue::Repeat(value, len) => {
                            Value::Array(vec![read(&locals, value); *len as usize])
                        }
                        Rvalue::Struct(fields) => {
                            Value::Struct(fields.iter().map(|e| read(&locals, e)).collect())
                        }
                        Rvalue::Read(local, path) => {
                            let steps = eval_path(&locals, path);
                            let mut value = read(&locals, &Operand::Local(*local));
                            part(&mut value, &steps).clone()
                        }
                    };
                    locals[dest.0] = Some(value);
                }

                Stmt::SetPlace { local, path, value } => {
                    let steps = eval_path(&locals, path);
                    let value = read(&locals, value);
                    let Some(target) = &mut locals[local.0] else {
                        panic!("{}: partial write to unassigned local {local}", function.name);
                    };
                    *part(target, &steps) = value;
                }

                Stmt::Call { func, args, dests } => {
                    let args = args
                        .iter()
                        .map(|arg| match arg {
                            CallArg::Value(operand) => read(&locals, operand),
                            CallArg::Place(place) => {
                                let steps = eval_path(&locals, &place.path);
                                let mut value = read(&locals, &Operand::Local(place.local));
                                part(&mut value, &steps).clone()
                            }
                        })
                        .collect();

                    // Destination indexes are evaluated before the call.
                    let dest_steps = dests
                        .iter()
                        .map(|dest| eval_path(&locals, &dest.path))
                        .collect::<Vec<_>>();

                    let results = call(program, *func, args);

                    assert_eq!(results.len(), dests.len());
                    for ((dest, steps), result) in dests.iter().zip(dest_steps).zip(results) {
                        match &mut locals[dest.local.0] {
                            Some(target) => *part(target, &steps) = result,
                            target @ None if steps.is_empty() => *target = Some(result),
                            None => panic!("partial write to unassigned local {}", dest.local),
                        }
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
    #[should_panic(expected = "index out of bounds")]
    fn out_of_bounds_index_panics() {
        let program = lower_source(
            "fn get(a: [i32; 2], i: i32): i32 { return a[i] }
             fn main(): i32 { return get([1, 2], 2) }",
        );

        run(&program);
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
            [(
                vec![CallArg::Value(Operand::Local(LocalId(x)))],
                vec![Place::whole(LocalId(x))]
            )]
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
