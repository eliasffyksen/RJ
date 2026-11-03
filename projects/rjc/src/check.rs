//! Name resolution and type checking: turns the AST into [`hir`].
//!
//! Besides resolving names and checking types, this pass enforces:
//! - every variable is assigned before it is read (definite assignment);
//! - every function with return values returns on all paths;
//! - no statement follows a `return` in the same block;
//! - a `fn main(): i32` exists.
//!
//! Errors are collected rather than stopping at the first one. Once part of
//! the program fails to check, it is dropped, so errors don't cascade.

use std::collections::{HashMap, HashSet};

use crate::{
    diagnostic::{Error, Span},
    hir::{self, BinOp, FuncId, LocalId, Scalar, Type},
    syntax::ast,
};

pub fn check(module: &ast::Module) -> Result<hir::Program, Vec<Error>> {
    let mut errors = vec![];
    let mut signatures = vec![];
    let mut by_name: HashMap<&str, FuncId> = HashMap::new();

    for (index, function) in module.functions.iter().enumerate() {
        let name = function.name.name.as_str();

        if by_name.contains_key(name) {
            errors.push(Error::new(
                format!("function `{name}` is already defined"),
                function.name.span,
            ));
        } else {
            by_name.insert(name, FuncId(index));
        }

        signatures.push(Signature {
            params: function.params.iter().map(|param| param.ty).collect(),
            returns: function.returns.clone(),
        });
    }

    let main = by_name.get("main").copied();

    match main {
        Some(id) => {
            let signature = &signatures[id.0];

            if !signature.params.is_empty() || signature.returns != [Type::I32] {
                errors.push(Error::new(
                    "`main` must be declared as `fn main(): i32`",
                    module.functions[id.0].name.span,
                ));
            }
        }
        None => errors.push(Error::new("no `main` function defined", Span::new(0, 0))),
    }

    let functions = module
        .functions
        .iter()
        .map(|function| {
            FunctionChecker {
                signatures: &signatures,
                by_name: &by_name,
                errors: &mut errors,
                returns: &function.returns,
                locals: vec![],
                scopes: vec![],
                assigned: HashSet::new(),
            }
            .function(function)
        })
        .collect::<Vec<_>>();

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(hir::Program {
        functions: functions.into_iter().map(Option::unwrap).collect(),
        main: main.unwrap(),
    })
}

struct Signature {
    params: Vec<Type>,
    returns: Vec<Type>,
}

struct FunctionChecker<'a> {
    signatures: &'a [Signature],
    by_name: &'a HashMap<&'a str, FuncId>,
    errors: &'a mut Vec<Error>,
    returns: &'a [Type],
    locals: Vec<hir::Local>,
    scopes: Vec<HashMap<String, LocalId>>,
    /// Locals that are definitely assigned at the current point.
    assigned: HashSet<LocalId>,
}

/// A checked statement (`None` for declarations, which only add a local)
/// and whether control never continues past it.
type StmtResult = Option<(Option<hir::Stmt>, bool)>;

impl FunctionChecker<'_> {
    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(Error::new(message, span));
    }

    fn function(mut self, function: &ast::Function) -> Option<hir::Function> {
        // Parameters and the top-level statements of the body share a scope,
        // so a body declaration can't shadow a parameter.
        self.scopes.push(HashMap::new());

        let mut params = vec![];
        for param in &function.params {
            if let Some(id) = self.declare(param) {
                self.assigned.insert(id);
                params.push(id);
            }
        }

        let (body, diverges) = self.stmts(&function.body)?;

        if !diverges && !function.returns.is_empty() {
            self.error(
                format!(
                    "function `{}` can reach its end without returning a value",
                    function.name.name
                ),
                function.name.span,
            );
            return None;
        }

        if params.len() != function.params.len() {
            return None;
        }

        Some(hir::Function {
            name: function.name.name.clone(),
            params,
            returns: function.returns.clone(),
            locals: self.locals,
            body,
            span: function.span,
        })
    }

    fn declare(&mut self, decl: &ast::VarDecl) -> Option<LocalId> {
        let name = &decl.name.name;
        let scope = self.scopes.last().unwrap();

        if scope.contains_key(name) {
            self.error(
                format!("`{name}` is already declared in this scope"),
                decl.name.span,
            );
            return None;
        }

        let id = LocalId(self.locals.len());

        self.locals.push(hir::Local {
            name: name.clone(),
            ty: decl.ty,
            span: decl.span,
        });
        self.scopes.last_mut().unwrap().insert(name.clone(), id);

        Some(id)
    }

    fn lookup(&mut self, ident: &ast::Ident) -> Option<LocalId> {
        let name = ident.name.as_str();

        if let Some(id) = self.scopes.iter().rev().find_map(|scope| scope.get(name)) {
            return Some(*id);
        }

        if self.by_name.contains_key(name) {
            self.error(format!("`{name}` is a function, not a variable"), ident.span);
        } else {
            self.error(format!("unknown variable `{name}`"), ident.span);
        }

        None
    }

    /// Checks a block in a new scope.
    fn block(&mut self, block: &ast::Block) -> Option<(hir::Block, bool)> {
        self.scopes.push(HashMap::new());
        let result = self.stmts(block);
        self.scopes.pop();
        result
    }

    /// Checks the statements of a block in the current scope.
    fn stmts(&mut self, block: &ast::Block) -> Option<(hir::Block, bool)> {
        let mut stmts = vec![];
        let mut ok = true;
        let mut diverges = false;

        for stmt in &block.stmts {
            if diverges {
                self.error("unreachable statement after `return`", stmt.span);
                return None;
            }

            match self.stmt(stmt) {
                Some((stmt, stmt_diverges)) => {
                    stmts.extend(stmt);
                    diverges = stmt_diverges;
                }
                None => ok = false,
            }
        }

        ok.then_some((hir::Block { stmts }, diverges))
    }

    fn stmt(&mut self, stmt: &ast::Stmt) -> StmtResult {
        let span = stmt.span;

        let (kind, diverges) = match &stmt.kind {
            ast::StmtKind::Decl(decl) => {
                self.declare(decl)?;
                return Some((None, false));
            }

            ast::StmtKind::Assign { targets, values } => {
                let values = self.values(values);

                let mut places = vec![];
                let mut expected = vec![];
                let mut whole = vec![];
                let mut ok = true;

                for target in targets {
                    let Some(id) = self.lookup(&target.name) else {
                        ok = false;
                        continue;
                    };
                    let ty = self.locals[id.0].ty;

                    if target.index.is_none() && whole.contains(&id) {
                        self.error(
                            format!("`{}` is assigned more than once", target.name.name),
                            target.span,
                        );
                        ok = false;
                    }

                    let Some(index) = &target.index else {
                        whole.push(id);
                        places.push(hir::Place::Local(id));
                        expected.push(ty);
                        continue;
                    };

                    // Writing one element keeps the others, so the array
                    // must already hold a value.
                    if !self.assigned.contains(&id) {
                        self.error(
                            format!("`{}` is used before it is assigned", target.name.name),
                            target.name.span,
                        );
                        ok = false;
                    }

                    let Type::Array(elem, len) = ty else {
                        self.error(
                            format!("cannot index into `{}` of type {ty}", target.name.name),
                            target.span,
                        );
                        ok = false;
                        continue;
                    };

                    match self.index(index, len) {
                        Some(index) => {
                            places.push(hir::Place::Index(id, index));
                            expected.push(elem.ty());
                        }
                        None => ok = false,
                    }
                }

                // `a, a[0] = ...` writes `a` twice.
                for target in targets {
                    if target.index.is_some()
                        && self.scopes.iter().rev().find_map(|s| s.get(&target.name.name))
                            .is_some_and(|id| whole.contains(id))
                    {
                        self.error(
                            format!("`{}` is assigned more than once", target.name.name),
                            target.span,
                        );
                        ok = false;
                    }
                }

                // Mark the targets as assigned even if the right-hand side
                // failed, so we don't also report every later use of them.
                self.assigned.extend(whole.iter().copied());

                let (values, types) = values?;

                if !ok {
                    return None;
                }

                self.expect_values(&expected, &types, span, "assignment", "value")?;

                (
                    hir::StmtKind::Assign {
                        targets: places,
                        values,
                    },
                    false,
                )
            }

            ast::StmtKind::Call(call) => (hir::StmtKind::Call(self.call(call, span)?), false),

            ast::StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond = self.condition(cond);

                let before = self.assigned.clone();
                let then_block = self.block(then_block);
                let after_then = std::mem::replace(&mut self.assigned, before);

                let else_block = match else_block {
                    Some(else_block) => self.block(else_block),
                    None => Some((hir::Block::default(), false)),
                };
                let after_else = std::mem::take(&mut self.assigned);

                let then_diverges = then_block.as_ref().is_some_and(|(_, d)| *d);
                let else_diverges = else_block.as_ref().is_some_and(|(_, d)| *d);

                // A branch that returns doesn't reach the code after the `if`,
                // so it doesn't limit what is assigned there.
                self.assigned = match (then_diverges, else_diverges) {
                    (true, true) => after_then,
                    (true, false) => after_else,
                    (false, true) => after_then,
                    (false, false) => after_then.intersection(&after_else).copied().collect(),
                };

                let (then_block, _) = then_block?;
                let (else_block, _) = else_block?;

                (
                    hir::StmtKind::If {
                        cond: cond?,
                        then_block,
                        else_block,
                    },
                    then_diverges && else_diverges,
                )
            }

            ast::StmtKind::While { cond, body } => {
                let cond = self.condition(cond);

                // The body may run zero times, so assignments in it don't
                // count after the loop. Every iteration starts with at least
                // what was assigned before the loop, so checking the body
                // against that is enough.
                let before = self.assigned.clone();
                let body = self.block(body);
                self.assigned = before;

                // A loop is never treated as returning, even `while true`.
                (
                    hir::StmtKind::While {
                        cond: cond?,
                        body: body?.0,
                    },
                    false,
                )
            }

            ast::StmtKind::Return(values) => {
                let (values, types) = self.values(values)?;
                let returns = self.returns;

                self.expect_values(returns, &types, span, "return", "value")?;

                (hir::StmtKind::Return(values), true)
            }
        };

        Some((Some(hir::Stmt { kind, span }), diverges))
    }

    fn condition(&mut self, cond: &ast::Expr) -> Option<hir::Expr> {
        let cond = self.expr(cond)?;

        if cond.ty != Type::Bool {
            self.error(
                format!("condition must be bool, found {}", cond.ty),
                cond.span,
            );
            return None;
        }

        Some(cond)
    }

    /// Checks a value list. Calls that don't return exactly one value
    /// contribute all their return values. Returns the values along with the
    /// type and source span of each individual value.
    fn values(&mut self, items: &[ast::Expr]) -> Option<(Vec<hir::Value>, Vec<(Type, Span)>)> {
        let mut values = vec![];
        let mut types = vec![];
        let mut ok = true;

        for item in items {
            if let ast::ExprKind::Call(call) = &item.kind {
                let func = self.by_name.get(call.callee.name.as_str()).copied();

                if let Some(func) = func {
                    let signatures = self.signatures;
                    let returns = &signatures[func.0].returns;

                    if returns.len() != 1 {
                        types.extend(returns.iter().map(|ty| (*ty, item.span)));

                        match self.call(call, item.span) {
                            Some(call) => values.push(hir::Value::Multi(call)),
                            None => ok = false,
                        }

                        continue;
                    }
                }
            }

            match self.expr(item) {
                Some(expr) => {
                    types.push((expr.ty, expr.span));
                    values.push(hir::Value::Single(expr));
                }
                None => ok = false,
            }
        }

        ok.then_some((values, types))
    }

    fn expect_values(
        &mut self,
        expected: &[Type],
        found: &[(Type, Span)],
        span: Span,
        what: &str,
        noun: &str,
    ) -> Option<()> {
        if expected.len() != found.len() {
            let plural = if expected.len() == 1 { "" } else { "s" };

            self.error(
                format!(
                    "{what} expects {} {noun}{plural}, found {}",
                    expected.len(),
                    found.len()
                ),
                span,
            );
            return None;
        }

        let mut ok = true;

        for (expected, (found, span)) in expected.iter().zip(found) {
            if expected != found {
                self.error(format!("expected {expected}, found {found}"), *span);
                ok = false;
            }
        }

        ok.then_some(())
    }

    fn call(&mut self, call: &ast::Call, span: Span) -> Option<hir::Call> {
        let name = call.callee.name.as_str();

        let Some(&func) = self.by_name.get(name) else {
            let is_variable = self.scopes.iter().any(|scope| scope.contains_key(name));

            if is_variable {
                self.error(format!("`{name}` is a variable, not a function"), call.callee.span);
            } else {
                self.error(format!("unknown function `{name}`"), call.callee.span);
            }

            return None;
        };

        let (args, types) = self.values(&call.args)?;
        let signatures = self.signatures;
        let params = &signatures[func.0].params;

        self.expect_values(params, &types, span, &format!("`{name}`"), "argument")?;

        Some(hir::Call { func, args, span })
    }

    fn expr(&mut self, expr: &ast::Expr) -> Option<hir::Expr> {
        let span = expr.span;

        let (kind, ty) = match &expr.kind {
            ast::ExprKind::Int(value) => (hir::ExprKind::Int(*value), Type::I32),

            ast::ExprKind::Bool(value) => (hir::ExprKind::Bool(*value), Type::Bool),

            ast::ExprKind::Var(ident) => {
                let id = self.lookup(ident)?;

                if !self.assigned.contains(&id) {
                    self.error(
                        format!("`{}` is used before it is assigned", ident.name),
                        ident.span,
                    );
                    return None;
                }

                (hir::ExprKind::Local(id), self.locals[id.0].ty)
            }

            ast::ExprKind::Call(call) => {
                let name = &call.callee.name;
                let call = self.call(call, span)?;
                let signatures = self.signatures;
                let returns = &signatures[call.func.0].returns;

                if returns.len() != 1 {
                    self.error(
                        format!(
                            "`{name}` returns {} values, but one value is expected here",
                            returns.len()
                        ),
                        span,
                    );
                    return None;
                }

                let ty = returns[0];
                (hir::ExprKind::Call(call), ty)
            }

            ast::ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.expr(lhs);
                let rhs = self.expr(rhs);
                let (lhs, rhs) = (lhs?, rhs?);

                let ty = match (op, lhs.ty, rhs.ty) {
                    (BinOp::Add | BinOp::Sub, Type::I32, Type::I32) => Type::I32,
                    (BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, Type::I32, Type::I32) => {
                        Type::Bool
                    }
                    (BinOp::Eq | BinOp::Ne, l, r) if l == r && l.scalar().is_some() => {
                        Type::Bool
                    }

                    (op, l, r) => {
                        self.error(
                            format!("cannot apply `{}` to {l} and {r}", op.as_str()),
                            span,
                        );
                        return None;
                    }
                };

                (
                    hir::ExprKind::Binary {
                        op: *op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ty,
                )
            }
            ast::ExprKind::ArrayList(elems) => {
                let mut checked = vec![];
                let mut ok = true;

                for elem in elems {
                    match self.expr(elem) {
                        Some(elem) => checked.push(elem),
                        None => ok = false,
                    }
                }

                if !ok {
                    return None;
                }

                let Some(first) = checked.first() else {
                    self.error(
                        "the element type of an empty array can't be inferred; \
                         use a repeat like `[0; 0]`",
                        span,
                    );
                    return None;
                };

                let elem = self.element_type(first.ty, first.span)?;

                for other in &checked[1..] {
                    if other.ty != elem.ty() {
                        self.error(format!("expected {}, found {}", elem.ty(), other.ty), other.span);
                        ok = false;
                    }
                }

                if !ok {
                    return None;
                }

                let len = checked.len() as u32;
                (hir::ExprKind::ArrayList(checked), Type::Array(elem, len))
            }

            ast::ExprKind::ArrayRepeat(value, len) => {
                let value = self.expr(value)?;
                let elem = self.element_type(value.ty, value.span)?;

                (
                    hir::ExprKind::ArrayRepeat(Box::new(value), *len),
                    Type::Array(elem, *len),
                )
            }

            ast::ExprKind::Index { base, index } => {
                let base = self.expr(base)?;

                let Type::Array(elem, len) = base.ty else {
                    self.error(format!("cannot index into a value of type {}", base.ty), base.span);
                    return None;
                };

                let index = self.index(index, len)?;

                (
                    hir::ExprKind::Index {
                        base: Box::new(base),
                        index: Box::new(index),
                    },
                    elem.ty(),
                )
            }
        };

        Some(hir::Expr { kind, ty, span })
    }

    fn element_type(&mut self, ty: Type, span: Span) -> Option<Scalar> {
        let scalar = ty.scalar();

        if scalar.is_none() {
            self.error(format!("array elements must be i32 or bool, found {ty}"), span);
        }

        scalar
    }

    /// Checks an index into an array of length `len`.
    fn index(&mut self, index: &ast::Expr, len: u32) -> Option<hir::Expr> {
        let index = self.expr(index)?;

        if index.ty != Type::I32 {
            self.error(format!("array index must be i32, found {}", index.ty), index.span);
            return None;
        }

        if let hir::ExprKind::Int(value) = index.kind {
            if value < 0 || value as u32 >= len {
                self.error(
                    format!("index {value} is out of bounds for an array of length {len}"),
                    index.span,
                );
                return None;
            }
        }

        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax;

    fn check_source(source: &str) -> Result<hir::Program, Vec<Error>> {
        check(&syntax::parse(source).unwrap())
    }

    /// Checks `source`, expecting exactly one error. Returns its message and
    /// the source text it points at.
    fn error(source: &str) -> (String, &str) {
        let errors = check_source(source).expect_err("expected an error");
        assert_eq!(errors.len(), 1, "expected one error, got {errors:#?}");

        let error = &errors[0];
        (error.message.clone(), &source[error.span.start..error.span.end])
    }

    #[test]
    fn checks_all_test_programs() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test");

        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let source = std::fs::read_to_string(&path).unwrap();
            let module = syntax::parse(&source).unwrap();

            if let Err(errors) = check(&module) {
                let path = path.display().to_string();
                let rendered = errors.iter().map(|e| e.render(&path, &source));
                panic!("{}", rendered.collect::<String>());
            }
        }
    }

    #[test]
    fn multiple_return_values() {
        let program = check_source(
            "fn pair(): i32, bool { return 1, true }
             fn main(): i32 {
               a: i32
               b: bool
               a, b = pair()
               return a
             }",
        )
        .unwrap();

        let main = program.function(program.main);
        let hir::StmtKind::Assign { values, .. } = &main.body.stmts[0].kind else {
            panic!("expected assignment");
        };
        assert!(matches!(values[..], [hir::Value::Multi(_)]));
    }

    #[test]
    fn unknown_variable() {
        let (message, at) = error("fn main(): i32 { return x }");
        assert_eq!(message, "unknown variable `x`");
        assert_eq!(at, "x");
    }

    #[test]
    fn use_before_assignment() {
        let (message, at) = error("fn main(): i32 { x: i32 return x }");
        assert_eq!(message, "`x` is used before it is assigned");
        assert_eq!(at, "x");
    }

    #[test]
    fn assignment_in_one_branch_only() {
        let (message, _) = error(
            "fn main(): i32 {
               x: i32
               if true { x = 1 }
               return x
             }",
        );
        assert_eq!(message, "`x` is used before it is assigned");
    }

    #[test]
    fn assignment_when_other_branch_returns() {
        check_source(
            "fn main(): i32 {
               x: i32
               if true { x = 1 } else { return 1 }
               return x
             }",
        )
        .unwrap();
    }

    #[test]
    fn assignment_in_loop_body_only() {
        let (message, _) = error(
            "fn main(): i32 {
               x: i32
               while false { x = 1 }
               return x
             }",
        );
        assert_eq!(message, "`x` is used before it is assigned");
    }

    #[test]
    fn loop_condition_must_be_bool() {
        let (message, at) = error("fn main(): i32 { while 0 { } return 0 }");
        assert_eq!(message, "condition must be bool, found i32");
        assert_eq!(at, "0");
    }

    #[test]
    fn constant_index_out_of_bounds() {
        let (message, at) = error("fn main(): i32 { a: [i32; 3] a = [0; 3] return a[3] }");
        assert_eq!(message, "index 3 is out of bounds for an array of length 3");
        assert_eq!(at, "3");
    }

    #[test]
    fn index_must_be_i32() {
        let (message, _) = error("fn main(): i32 { a: [i32; 3] a = [0; 3] return a[true] }");
        assert_eq!(message, "array index must be i32, found bool");
    }

    #[test]
    fn indexing_a_scalar() {
        let (message, _) = error("fn main(): i32 { a: i32 a = 1 return a[0] }");
        assert_eq!(message, "cannot index into a value of type i32");
    }

    #[test]
    fn element_assignment_needs_assigned_array() {
        let (message, at) = error("fn main(): i32 { a: [i32; 3] a[0] = 1 return 0 }");
        assert_eq!(message, "`a` is used before it is assigned");
        assert_eq!(at, "a");
    }

    #[test]
    fn mixed_element_types() {
        let (message, at) = error("fn main(): i32 { a: [i32; 2] a = [1, true] return 0 }");
        assert_eq!(message, "expected i32, found bool");
        assert_eq!(at, "true");
    }

    #[test]
    fn array_length_mismatch() {
        let (message, _) = error("fn main(): i32 { a: [i32; 2] a = [1, 2, 3] return 0 }");
        assert_eq!(message, "expected [i32; 2], found [i32; 3]");
    }

    #[test]
    fn empty_array_literal() {
        let (message, _) = error("fn main(): i32 { a: [i32; 0] a = [] return 0 }");
        assert!(message.starts_with("the element type of an empty array can't be inferred"));
    }

    #[test]
    fn arrays_cannot_be_compared() {
        let (message, _) = error(
            "fn main(): i32 { a: [i32; 1] a = [1] if a == a { return 1 } return 0 }",
        );
        assert_eq!(message, "cannot apply `==` to [i32; 1] and [i32; 1]");
    }

    #[test]
    fn whole_and_element_assignment_together() {
        for targets in ["a, a[0]", "a[0], a"] {
            let source = format!(
                "fn main(): i32 {{ a: [i32; 1] a = [1] {targets} = [2], 3 return 0 }}"
            );
            let errors = check_source(&source).unwrap_err();
            let messages = errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>();

            assert_eq!(messages, ["`a` is assigned more than once"], "{targets}");
        }
    }

    #[test]
    fn missing_return() {
        let (message, at) = error("fn main(): i32 { if true { return 1 } }");
        assert_eq!(message, "function `main` can reach its end without returning a value");
        assert_eq!(at, "main");
    }

    #[test]
    fn unreachable_statement() {
        let (message, _) = error("fn main(): i32 { return 1 return 2 }");
        assert_eq!(message, "unreachable statement after `return`");
    }

    #[test]
    fn operand_types() {
        let (message, at) = error("fn main(): i32 { return 1 + true }");
        assert_eq!(message, "cannot apply `+` to i32 and bool");
        assert_eq!(at, "1 + true");
    }

    #[test]
    fn argument_count() {
        let (message, _) = error(
            "fn f(a: i32): i32 { return a }
             fn main(): i32 { return f(1, 2) }",
        );
        assert_eq!(message, "`f` expects 1 argument, found 2");
    }

    #[test]
    fn argument_type() {
        let (message, at) = error(
            "fn f(a: i32): i32 { return a }
             fn main(): i32 { return f(false) }",
        );
        assert_eq!(message, "expected i32, found bool");
        assert_eq!(at, "false");
    }

    #[test]
    fn multi_value_call_in_expression() {
        let (message, _) = error(
            "fn pair(): i32, i32 { return 1, 2 }
             fn main(): i32 { return pair() + 1 }",
        );
        assert_eq!(message, "`pair` returns 2 values, but one value is expected here");
    }

    #[test]
    fn condition_must_be_bool() {
        let (message, at) = error("fn main(): i32 { if 1 { return 1 } return 0 }");
        assert_eq!(message, "condition must be bool, found i32");
        assert_eq!(at, "1");
    }

    #[test]
    fn redeclaration() {
        let (message, _) = error("fn main(): i32 { x: i32 x: i32 x = 1 return x }");
        assert_eq!(message, "`x` is already declared in this scope");
    }

    #[test]
    fn shadowing_in_nested_block() {
        check_source(
            "fn main(): i32 {
               x: i32
               x = 1
               if true { x: bool x = false }
               return x
             }",
        )
        .unwrap();
    }

    #[test]
    fn missing_main() {
        let (message, _) = error("fn f() {}");
        assert_eq!(message, "no `main` function defined");
    }

    #[test]
    fn errors_do_not_cascade() {
        let errors = check_source(
            "fn main(): i32 {
               x: i32
               x = y
               return x + z
             }",
        )
        .unwrap_err();

        let messages = errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>();
        assert_eq!(messages, ["unknown variable `y`", "unknown variable `z`"]);
    }
}
