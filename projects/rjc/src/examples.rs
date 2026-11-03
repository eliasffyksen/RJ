//! Runs a program's `example`s with the IR interpreter.

use crate::{
    diagnostic::{Code, Error},
    ir::{
        interp::{format_value, Machine, Value},
        BinOp, Program,
    },
};

/// Statements an example may run before it is stopped.
const STEPS: u64 = 10_000_000;

/// Runs every example, returning an error for each one that doesn't hold.
pub fn run(program: &Program) -> Vec<Error> {
    let mut errors = vec![];

    for example in &program.examples {
        let mut machine = Machine::new(program, STEPS);

        let values = match machine.run(&example.function, vec![]) {
            Ok(values) => values,
            Err(trap) => {
                errors.push(Error::new(
                    Code::ExampleTrapped,
                    format!("example stopped: {trap}"),
                    example.span,
                ));
                continue;
            }
        };

        let show = |i: usize| format_value(&values[i], &example.function.local(example.function.returns[i]).ty, &program.structs);

        match example.compare {
            Some(op) => {
                let equal = values[0] == values[1];

                if equal != (op == BinOp::Eq) {
                    let (left, right) = (show(0), show(1));
                    let mut error = Error::new(Code::ExampleFailed, "example failed", example.span);

                    if op == BinOp::Eq {
                        error.help(format!("left side is `{left}`, right side is `{right}`"));
                        error.types(&right, &left);
                    } else {
                        error.help(format!("both sides are `{left}`"));
                    }

                    errors.push(error);
                }
            }
            None => {
                if values[0] == Value::Bool(false) {
                    errors.push(Error::new(
                        Code::ExampleFailed,
                        "example failed: the expression is false",
                        example.span,
                    ));
                }
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, ir, syntax};

    fn run_source(source: &str) -> Vec<Error> {
        let program = check::check(&syntax::parse(source).unwrap()).unwrap();
        run(&ir::lower(&program))
    }

    const MAIN: &str = "fn main(): i32 { return 0 }";

    #[test]
    fn passing_examples_report_nothing() {
        let source = format!("fn sq(x: i32): i32 {{ return x + x }} example sq(3) == 6 {MAIN}");
        assert!(run_source(&source).is_empty());
    }

    #[test]
    fn failing_comparison_shows_both_sides() {
        let source = format!("fn sq(x: i32): i32 {{ return x + x }} example sq(3) == 9 {MAIN}");
        let errors = run_source(&source);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, Code::ExampleFailed);
        assert_eq!(errors[0].help.as_deref(), Some("left side is `6`, right side is `9`"));
        assert_eq!(errors[0].expected.as_deref(), Some("9"));
        assert_eq!(errors[0].found.as_deref(), Some("6"));
    }

    #[test]
    fn structs_and_arrays_are_shown_in_rj_syntax() {
        let source = format!(
            "type P struct {{ a: [i32; 2], b: bool }}
             fn make(): P {{ return P(a: [1, 2], b: true) }}
             example make() == P(a: [1, 3], b: true)
             {MAIN}"
        );
        let errors = run_source(&source);

        assert_eq!(
            errors[0].help.as_deref(),
            Some("left side is `P(a: [1, 2], b: true)`, right side is `P(a: [1, 3], b: true)`")
        );
    }

    #[test]
    fn false_bool_example() {
        let source = format!("example 1 > 2 {MAIN}");
        let errors = run_source(&source);
        assert_eq!(errors[0].message, "example failed: the expression is false");
    }

    #[test]
    fn traps_are_reported() {
        let source = format!(
            "fn get(a: [i32; 2], i: i32): i32 {{ return a[i] }} example get([1, 2], 5) == 0 {MAIN}"
        );
        let errors = run_source(&source);

        assert_eq!(errors[0].code, Code::ExampleTrapped);
        assert_eq!(errors[0].message, "example stopped: index out of bounds: 5 for length 2");
    }

    #[test]
    fn endless_loops_are_stopped() {
        let source = format!(
            "fn spin(): i32 {{ n: i32 n = 0 while true {{ n = n + 1 }} return n }}
             example spin() == 0 {MAIN}"
        );
        let errors = run_source(&source);

        assert_eq!(errors[0].code, Code::ExampleTrapped);
        assert!(errors[0].message.contains("ran out of steps"));
    }
}
