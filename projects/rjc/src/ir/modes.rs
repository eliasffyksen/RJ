//! Parameter modes: how a function uses each of its array parameters.
//!
//! RJ passes everything by value, but copying arrays is expensive. Each
//! function's array parameters are classified from its own body, and both
//! the function and its callers use the result to decide when a copy can be
//! skipped (see `codegen`):
//!
//! - [`ParamMode::InOut`]: every `return` hands back the parameter itself in
//!   the same return slot. The parameter and that return slot share storage,
//!   so the caller passes a single buffer holding the argument, which the
//!   callee updates in place and which then holds the result. `x = f(x)`
//!   needs no copy at all.
//! - [`ParamMode::Read`]: the parameter is never written, so the caller can
//!   lend its own array instead of copying it.
//! - [`ParamMode::Consume`]: the parameter is written. The callee owns the
//!   buffer it receives; the caller hands over its own array when it no
//!   longer needs the value, and passes a copy otherwise.
//!
//! The classification only looks at the function's own body. Return slots
//! are only written by `return`, so the in/out check is a direct look at
//! what each `return` writes.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    /// Scalars, passed in registers.
    Value,
    Read,
    Consume,
    /// Shares storage with `returns[slot]`.
    InOut(usize),
}

impl fmt::Display for ParamMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamMode::Value => f.write_str("value"),
            ParamMode::Read => f.write_str("read"),
            ParamMode::Consume => f.write_str("consume"),
            ParamMode::InOut(slot) => write!(f, "in/out (return {slot})"),
        }
    }
}

pub fn param_modes(function: &Function) -> Vec<ParamMode> {
    let mut written = vec![false; function.locals.len()];
    // For each return slot: the local copied into it by every write, if
    // there is a single such local.
    let mut slot_source: Vec<Option<Option<LocalId>>> = vec![None; function.returns.len()];

    let mut record_slot_write = |dest: LocalId, source: Option<LocalId>| {
        if let Some(slot) = function.returns.iter().position(|r| *r == dest) {
            slot_source[slot] = match slot_source[slot] {
                None => Some(source),
                Some(previous) if previous == source => Some(source),
                Some(_) => Some(None),
            };
        }
    };

    for block in &function.blocks {
        for stmt in &block.stmts {
            match stmt {
                Stmt::Assign { dest, value } => {
                    written[dest.0] = true;

                    let source = match value {
                        Rvalue::Use(Operand::Local(src)) => Some(*src),
                        _ => None,
                    };
                    record_slot_write(*dest, source);
                }
                Stmt::SetIndex { array, .. } => written[array.0] = true,
                Stmt::Call { dests, .. } => {
                    for dest in dests {
                        written[dest.0] = true;
                        record_slot_write(*dest, None);
                    }
                }
            }
        }
    }

    let mut paired = vec![false; function.returns.len()];

    function
        .params
        .iter()
        .map(|param| {
            if !matches!(function.local(*param).ty, Type::Array(..)) {
                return ParamMode::Value;
            }

            let slot = (0..function.returns.len())
                .find(|slot| !paired[*slot] && slot_source[*slot] == Some(Some(*param)));

            if let Some(slot) = slot {
                paired[slot] = true;
                ParamMode::InOut(slot)
            } else if written[param.0] {
                ParamMode::Consume
            } else {
                ParamMode::Read
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, syntax};

    fn modes(source: &str, name: &str) -> Vec<ParamMode> {
        let program = lower(&check::check(&syntax::parse(source).unwrap()).unwrap());
        let function = program.functions.iter().find(|f| f.name == name).unwrap();
        param_modes(function)
    }

    const MAIN: &str = "fn main(): i32 { return 0 }";

    #[test]
    fn read_only() {
        let source = format!("fn f(a: [i32; 2], i: i32): i32 {{ return a[i] }} {MAIN}");
        assert_eq!(modes(&source, "f"), [ParamMode::Read, ParamMode::Value]);
    }

    #[test]
    fn written_but_not_returned() {
        let source = format!("fn f(a: [i32; 2]): i32 {{ a[0] = 1 return a[0] }} {MAIN}");
        assert_eq!(modes(&source, "f"), [ParamMode::Consume]);
    }

    #[test]
    fn returned_in_place() {
        let source = format!("fn f(a: [i32; 2]): [i32; 2] {{ a[0] = 1 return a }} {MAIN}");
        assert_eq!(modes(&source, "f"), [ParamMode::InOut(0)]);
    }

    #[test]
    fn returned_on_every_path() {
        let source = format!(
            "fn f(a: [i32; 2], c: bool): i32, [i32; 2] {{
               if c {{ return 1, a }}
               a[0] = 2
               return 2, a
             }} {MAIN}"
        );
        assert_eq!(modes(&source, "f"), [ParamMode::InOut(1), ParamMode::Value]);
    }

    #[test]
    fn returned_on_some_paths_only() {
        let source = format!(
            "fn f(a: [i32; 2], c: bool): [i32; 2] {{
               if c {{ return a }}
               return [0; 2]
             }} {MAIN}"
        );
        assert_eq!(modes(&source, "f"), [ParamMode::Read, ParamMode::Value]);
    }

    #[test]
    fn swapped_returns() {
        let source = format!(
            "fn f(a: [i32; 2], b: [i32; 2]): [i32; 2], [i32; 2] {{ return b, a }} {MAIN}"
        );
        assert_eq!(modes(&source, "f"), [ParamMode::InOut(1), ParamMode::InOut(0)]);
    }

    #[test]
    fn same_param_in_two_slots() {
        let source = format!(
            "fn f(a: [i32; 2]): [i32; 2], [i32; 2] {{ return a, a }} {MAIN}"
        );
        assert_eq!(modes(&source, "f"), [ParamMode::InOut(0)]);
    }
}
