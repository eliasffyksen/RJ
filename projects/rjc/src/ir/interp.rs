//! An IR interpreter. It runs `example`s at compile time, and tests use it
//! to check that lowering preserves the meaning of programs independently
//! of any backend.
//!
//! Errors in the running program (an index out of bounds, or running out of
//! steps) are returned as [`Trap`]s. Anything else that goes wrong is a bug
//! in the compiler and panics.

use std::fmt;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Int(i32),
    Bool(bool),
    Array(Vec<Value>),
    Struct(Vec<Value>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trap {
    IndexOutOfBounds { index: i32, len: usize },
    /// The step budget ran out, probably in a loop that never ends.
    OutOfSteps,
}

impl fmt::Display for Trap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trap::IndexOutOfBounds { index, len } => {
                write!(f, "index out of bounds: {index} for length {len}")
            }
            Trap::OutOfSteps => f.write_str("ran out of steps; a loop may never end"),
        }
    }
}

/// An evaluated path step.
enum Step {
    Index(Value),
    Field(usize),
}

/// The part of `value` that `path` leads to.
fn part<'v>(value: &'v mut Value, path: &[Step]) -> Result<&'v mut Value, Trap> {
    let Some((step, rest)) = path.split_first() else {
        return Ok(value);
    };

    let inner = match (step, value) {
        (Step::Index(index), Value::Array(elems)) => {
            let i = array_index(elems, index)?;
            &mut elems[i]
        }
        (Step::Field(field), Value::Struct(fields)) => &mut fields[*field],
        (_, value) => panic!("path does not match value {value:?}"),
    };

    part(inner, rest)
}

fn array_index(array: &[Value], index: &Value) -> Result<usize, Trap> {
    match *index {
        Value::Int(i) if i >= 0 && (i as usize) < array.len() => Ok(i as usize),
        Value::Int(index) => Err(Trap::IndexOutOfBounds {
            index,
            len: array.len(),
        }),
        ref other => panic!("array index {other:?}"),
    }
}

/// Runs `main` and returns its exit code. Panics on a trap.
#[cfg(test)]
pub fn run(program: &Program) -> i32 {
    let mut machine = Machine::new(program, u64::MAX);

    match machine.call(program.main, vec![]) {
        Ok(values) => match values[..] {
            [Value::Int(code)] => code,
            ref other => panic!("main returned {other:?}"),
        },
        Err(trap) => panic!("{trap}"),
    }
}

pub struct Machine<'p> {
    program: &'p Program,
    steps_left: u64,
}

impl<'p> Machine<'p> {
    /// A machine that stops with [`Trap::OutOfSteps`] after `steps`
    /// statements.
    pub fn new(program: &'p Program, steps: u64) -> Self {
        Machine {
            program,
            steps_left: steps,
        }
    }

    pub fn call(&mut self, func: FuncId, args: Vec<Value>) -> Result<Vec<Value>, Trap> {
        let program = self.program;
        self.run(program.function(func), args)
    }

    fn step(&mut self) -> Result<(), Trap> {
        if self.steps_left == 0 {
            return Err(Trap::OutOfSteps);
        }
        self.steps_left -= 1;
        Ok(())
    }

    /// Runs `function`, which may be outside the program (an example), with
    /// calls going to the program's functions.
    pub fn run(&mut self, function: &Function, args: Vec<Value>) -> Result<Vec<Value>, Trap> {
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

        let read_place = |locals: &[Option<Value>], local: LocalId, path: &[Elem]| {
            let steps = eval_path(locals, path);
            let mut value = read(locals, &Operand::Local(local));
            Ok(part(&mut value, &steps)?.clone())
        };

        let mut block = BlockId(0);

        loop {
            let data = function.block(block);

            for stmt in &data.stmts {
                self.step()?;

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
                            Rvalue::Read(local, path) => read_place(&locals, *local, path)?,
                        };
                        locals[dest.0] = Some(value);
                    }

                    Stmt::SetPlace { local, path, value } => {
                        let steps = eval_path(&locals, path);
                        let value = read(&locals, value);
                        let Some(target) = &mut locals[local.0] else {
                            panic!("{}: partial write to unassigned local {local}", function.name);
                        };
                        *part(target, &steps)? = value;
                    }

                    Stmt::Call { func, args, dests } => {
                        let mut values = vec![];
                        for arg in args {
                            values.push(match arg {
                                CallArg::Value(operand) => read(&locals, operand),
                                CallArg::Place(place) => {
                                    read_place(&locals, place.local, &place.path)?
                                }
                            });
                        }

                        // Destination indexes are evaluated before the call.
                        let dest_steps = dests
                            .iter()
                            .map(|dest| eval_path(&locals, &dest.path))
                            .collect::<Vec<_>>();

                        let results = self.call(*func, values)?;

                        assert_eq!(results.len(), dests.len());
                        for ((dest, steps), result) in dests.iter().zip(dest_steps).zip(results) {
                            match &mut locals[dest.local.0] {
                                Some(target) => *part(target, &steps)? = result,
                                target @ None if steps.is_empty() => *target = Some(result),
                                None => panic!("partial write to unassigned local {}", dest.local),
                            }
                        }
                    }
                }
            }

            self.step()?;

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
                    return Ok(function
                        .returns
                        .iter()
                        .map(|slot| read(&locals, &Operand::Local(*slot)))
                        .collect());
                }
            }
        }
    }
}

/// `value` in RJ syntax, using `ty` for struct names and field names.
pub fn format_value(value: &Value, ty: &Type, structs: &[StructDef]) -> String {
    match (value, ty) {
        (Value::Int(v), _) => v.to_string(),
        (Value::Bool(v), _) => v.to_string(),
        (Value::Array(elems), Type::Array(elem_ty, _)) => {
            let elems = elems
                .iter()
                .map(|e| format_value(e, elem_ty, structs))
                .collect::<Vec<_>>();
            format!("[{}]", elems.join(", "))
        }
        (Value::Struct(fields), Type::Struct(id, name)) => {
            let def = &structs[id.0];
            let fields = fields
                .iter()
                .zip(&def.fields)
                .map(|(v, f)| format!("{}: {}", f.name, format_value(v, &f.ty, structs)))
                .collect::<Vec<_>>();
            format!("{name}({})", fields.join(", "))
        }
        (value, ty) => panic!("value {value:?} does not match type {ty}"),
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

            let failed = crate::examples::run(&program);
            assert!(failed.is_empty(), "{}: {failed:#?}", path.display());
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
