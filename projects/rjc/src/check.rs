//! Name resolution and type checking: turns the AST into [`hir`].
//!
//! Besides resolving names and checking types, this pass enforces:
//! - every variable is assigned before it is read (definite assignment);
//! - every function with return values returns on all paths;
//! - no statement follows a `return` in the same block;
//! - no struct contains itself;
//! - a `fn main(): i32` exists.
//!
//! Errors are collected rather than stopping at the first one. Once part of
//! the program fails to check, it is dropped, so errors don't cascade.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    diagnostic::{Error, Span},
    hir::{self, BinOp, FuncId, LocalId, StructDef, StructId, Type},
    syntax::ast,
    types::Field,
};

pub fn check(module: &ast::Module) -> Result<hir::Program, Vec<Error>> {
    let mut errors = vec![];

    let mut functions_by_name: HashMap<&str, FuncId> = HashMap::new();
    for (index, function) in module.functions.iter().enumerate() {
        let name = function.name.name.as_str();

        if functions_by_name.contains_key(name) {
            errors.push(Error::new(
                format!("function `{name}` is already defined"),
                function.name.span,
            ));
        } else {
            functions_by_name.insert(name, FuncId(index));
        }
    }

    let types = Types::new(module, &functions_by_name, &mut errors);

    // A signature is `None` if one of its types failed to resolve; calls to
    // it are then skipped rather than reported again.
    let signatures = module
        .functions
        .iter()
        .map(|function| {
            let params = function
                .params
                .iter()
                .map(|param| types.resolve(&param.ty, &mut errors))
                .collect::<Option<Vec<_>>>();
            let returns = function
                .returns
                .iter()
                .map(|ty| types.resolve(ty, &mut errors))
                .collect::<Option<Vec<_>>>();

            Some(Signature {
                params: params?,
                returns: returns?,
            })
        })
        .collect::<Vec<_>>();

    let main = functions_by_name.get("main").copied();

    match main {
        Some(id) => {
            let valid = signatures[id.0]
                .as_ref()
                .is_none_or(|sig| sig.params.is_empty() && sig.returns == [Type::I32]);

            if !valid {
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
        .zip(&signatures)
        .map(|(function, signature)| {
            let signature = signature.as_ref()?;

            FunctionChecker {
                types: &types,
                signatures: &signatures,
                functions_by_name: &functions_by_name,
                errors: &mut errors,
                returns: &signature.returns,
                locals: vec![],
                scopes: vec![],
                assigned: HashSet::new(),
            }
            .function(function, signature)
        })
        .collect::<Vec<_>>();

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(hir::Program {
        structs: types.structs,
        functions: functions.into_iter().map(Option::unwrap).collect(),
        main: main.unwrap(),
    })
}

struct Signature {
    params: Vec<Type>,
    returns: Vec<Type>,
}

/// The program's structs, and resolution of type syntax.
struct Types {
    structs: Vec<StructDef>,
    by_name: HashMap<String, StructId>,
}

impl Types {
    fn new(
        module: &ast::Module,
        functions_by_name: &HashMap<&str, FuncId>,
        errors: &mut Vec<Error>,
    ) -> Types {
        let mut types = Types {
            structs: vec![],
            by_name: HashMap::new(),
        };

        // Register every name first, so fields can refer to any struct.
        for def in &module.structs {
            let name = &def.name.name;

            if types.by_name.contains_key(name) {
                errors.push(Error::new(
                    format!("struct `{name}` is already defined"),
                    def.name.span,
                ));
            } else if functions_by_name.contains_key(name.as_str()) {
                // `Name(...)` must mean one thing.
                errors.push(Error::new(
                    format!("`{name}` is defined as both a struct and a function"),
                    def.name.span,
                ));
            }

            types
                .by_name
                .entry(name.clone())
                .or_insert(StructId(types.structs.len()));
            types.structs.push(StructDef {
                name: name.clone(),
                fields: vec![],
            });
        }

        for (index, def) in module.structs.iter().enumerate() {
            let mut fields: Vec<Field> = vec![];

            for field in &def.fields {
                if fields.iter().any(|f| f.name == field.name.name) {
                    errors.push(Error::new(
                        format!("field `{}` is already declared", field.name.name),
                        field.name.span,
                    ));
                    continue;
                }

                if let Some(ty) = types.resolve(&field.ty, errors) {
                    fields.push(Field {
                        name: field.name.name.clone(),
                        ty,
                    });
                }
            }

            types.structs[index].fields = fields;
        }

        for (index, def) in module.structs.iter().enumerate() {
            if types.contains(&mut vec![], StructId(index), StructId(index)) {
                errors.push(Error::new(
                    format!(
                        "struct `{}` contains itself, so it would have infinite size",
                        def.name.name
                    ),
                    def.name.span,
                ));
            }
        }

        types
    }

    /// Whether `outer` contains a `target` somewhere inside it.
    fn contains(&self, visited: &mut Vec<StructId>, outer: StructId, target: StructId) -> bool {
        if visited.contains(&outer) {
            return false;
        }
        visited.push(outer);

        self.structs[outer.0].fields.iter().any(|field| {
            let mut ty = &field.ty;
            while let Type::Array(elem, _) = ty {
                ty = elem;
            }

            match ty {
                Type::Struct(id, _) => *id == target || self.contains(visited, *id, target),
                _ => false,
            }
        })
    }

    fn resolve(&self, ty: &ast::TypeExpr, errors: &mut Vec<Error>) -> Option<Type> {
        match &ty.kind {
            ast::TypeExprKind::I32 => Some(Type::I32),
            ast::TypeExprKind::Bool => Some(Type::Bool),
            ast::TypeExprKind::Array(elem, len) => {
                Some(Type::Array(Box::new(self.resolve(elem, errors)?), *len))
            }
            ast::TypeExprKind::Named(name) => match self.by_name.get(&name.name) {
                Some(id) => Some(Type::Struct(*id, Rc::from(name.name.as_str()))),
                None => {
                    errors.push(Error::new(format!("unknown type `{}`", name.name), name.span));
                    None
                }
            },
        }
    }
}

struct FunctionChecker<'a> {
    types: &'a Types,
    signatures: &'a [Option<Signature>],
    functions_by_name: &'a HashMap<&'a str, FuncId>,
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

    fn function(mut self, function: &ast::Function, signature: &Signature) -> Option<hir::Function> {
        // Parameters and the top-level statements of the body share a scope,
        // so a body declaration can't shadow a parameter.
        self.scopes.push(HashMap::new());

        let mut params = vec![];
        for (param, ty) in function.params.iter().zip(&signature.params) {
            if let Some(id) = self.declare_typed(param, ty.clone()) {
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
            returns: signature.returns.clone(),
            locals: self.locals,
            body,
            span: function.span,
        })
    }

    fn declare(&mut self, decl: &ast::VarDecl) -> Option<LocalId> {
        let ty = self.types.resolve(&decl.ty, self.errors)?;
        self.declare_typed(decl, ty)
    }

    fn declare_typed(&mut self, decl: &ast::VarDecl, ty: Type) -> Option<LocalId> {
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
            ty,
            span: decl.span,
        });
        self.scopes.last_mut().unwrap().insert(name.clone(), id);

        Some(id)
    }

    fn find_local(&self, name: &str) -> Option<LocalId> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name)).copied()
    }

    fn lookup(&mut self, ident: &ast::Ident) -> Option<LocalId> {
        let name = ident.name.as_str();

        if let Some(id) = self.find_local(name) {
            return Some(id);
        }

        if self.functions_by_name.contains_key(name) {
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
                    if target.path.is_empty() {
                        if let Some(id) = self.find_local(&target.name.name) {
                            if whole.contains(&id) {
                                self.error(
                                    format!("`{}` is assigned more than once", target.name.name),
                                    target.span,
                                );
                                ok = false;
                            }
                            whole.push(id);
                        }
                    }

                    match self.place(target) {
                        Some((place, ty)) => {
                            places.push(place);
                            expected.push(ty);
                        }
                        None => ok = false,
                    }
                }

                // `a, a[0] = ...` writes `a` twice.
                for target in targets {
                    let root = self.find_local(&target.name.name);

                    if !target.path.is_empty() && root.is_some_and(|id| whole.contains(&id)) {
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

    /// Checks an assignment target, returning it with the type it holds.
    fn place(&mut self, target: &ast::Place) -> Option<(hir::Place, Type)> {
        let local = self.lookup(&target.name)?;
        let mut ty = self.locals[local.0].ty.clone();

        // Writing a part keeps the rest, so the variable must already hold a
        // value.
        if !target.path.is_empty() && !self.assigned.contains(&local) {
            self.error(
                format!("`{}` is used before it is assigned", target.name.name),
                target.name.span,
            );
            return None;
        }

        let mut path = vec![];

        for elem in &target.path {
            match elem {
                ast::PlaceElem::Index(index) => {
                    let Type::Array(elem, len) = ty else {
                        self.error(format!("cannot index into a value of type {ty}"), target.span);
                        return None;
                    };

                    path.push(hir::PlaceElem::Index(self.index(index, len)?));
                    ty = *elem;
                }

                ast::PlaceElem::Field(name) => {
                    let (field, field_ty) = self.field(&ty, name)?;
                    path.push(hir::PlaceElem::Field(field));
                    ty = field_ty;
                }
            }
        }

        Some((hir::Place { local, path }, ty))
    }

    /// Looks up field `name` on a value of type `ty`.
    fn field(&mut self, ty: &Type, name: &ast::Ident) -> Option<(usize, Type)> {
        let Type::Struct(id, _) = ty else {
            self.error(format!("{ty} has no field `{}`", name.name), name.span);
            return None;
        };

        let def = &self.types.structs[id.0];

        let Some(index) = def.field_index(&name.name) else {
            self.error(format!("{ty} has no field `{}`", name.name), name.span);
            return None;
        };

        Some((index, def.fields[index].ty.clone()))
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

    /// The return types of the function `name`, if it is one and its
    /// signature resolved.
    fn returns_of(&self, name: &str) -> Option<&[Type]> {
        let func = self.functions_by_name.get(name)?;
        let signature = self.signatures[func.0].as_ref()?;
        Some(&signature.returns)
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
                let returns = self.returns_of(&call.callee.name).map(<[Type]>::to_vec);

                if let Some(returns) = returns.filter(|returns| returns.len() != 1) {
                    types.extend(returns.into_iter().map(|ty| (ty, item.span)));

                    match self.call(call, item.span) {
                        Some(call) => values.push(hir::Value::Multi(call)),
                        None => ok = false,
                    }

                    continue;
                }
            }

            match self.expr(item) {
                Some(expr) => {
                    types.push((expr.ty.clone(), expr.span));
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

        let Some(&func) = self.functions_by_name.get(name) else {
            if self.find_local(name).is_some() {
                self.error(format!("`{name}` is a variable, not a function"), call.callee.span);
            } else if self.types.by_name.contains_key(name) {
                self.error(
                    format!("struct `{name}` is built with named fields: `{name}(field: value, ...)`"),
                    call.callee.span,
                );
            } else {
                self.error(format!("unknown function `{name}`"), call.callee.span);
            }

            return None;
        };

        let (args, types) = self.values(&call.args)?;

        // The signature didn't resolve; that was already reported.
        let signatures = self.signatures;
        let params = &signatures[func.0].as_ref()?.params;

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

                (hir::ExprKind::Local(id), self.locals[id.0].ty.clone())
            }

            // `Empty()` builds a struct without fields.
            ast::ExprKind::Call(call)
                if call.args.is_empty()
                    && !self.functions_by_name.contains_key(call.callee.name.as_str())
                    && self.types.by_name.contains_key(&call.callee.name) =>
            {
                return self.struct_lit(&call.callee, &[], span);
            }

            ast::ExprKind::Call(call) => {
                let name = &call.callee.name;
                let call = self.call(call, span)?;
                let returns = self.returns_of(name)?.to_vec();

                let [ty] = &returns[..] else {
                    self.error(
                        format!(
                            "`{name}` returns {} values, but one value is expected here",
                            returns.len()
                        ),
                        span,
                    );
                    return None;
                };

                (hir::ExprKind::Call(call), ty.clone())
            }

            ast::ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.expr(lhs);
                let rhs = self.expr(rhs);
                let (lhs, rhs) = (lhs?, rhs?);

                let ty = match (op, &lhs.ty, &rhs.ty) {
                    (BinOp::Add | BinOp::Sub, Type::I32, Type::I32) => Type::I32,
                    (BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, Type::I32, Type::I32) => {
                        Type::Bool
                    }
                    (BinOp::Eq | BinOp::Ne, l, r) if l == r && l.is_scalar() => Type::Bool,

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

                let elem = first.ty.clone();

                for other in &checked[1..] {
                    if other.ty != elem {
                        self.error(format!("expected {elem}, found {}", other.ty), other.span);
                        ok = false;
                    }
                }

                if !ok {
                    return None;
                }

                let len = checked.len() as u32;
                (hir::ExprKind::ArrayList(checked), Type::Array(Box::new(elem), len))
            }

            ast::ExprKind::ArrayRepeat(value, len) => {
                let value = self.expr(value)?;
                let ty = Type::Array(Box::new(value.ty.clone()), *len);

                (hir::ExprKind::ArrayRepeat(Box::new(value), *len), ty)
            }

            ast::ExprKind::Index { base, index } => {
                let base = self.expr(base)?;

                let Type::Array(elem, len) = base.ty.clone() else {
                    self.error(format!("cannot index into a value of type {}", base.ty), base.span);
                    return None;
                };

                let index = self.index(index, len)?;

                (
                    hir::ExprKind::Index {
                        base: Box::new(base),
                        index: Box::new(index),
                    },
                    *elem,
                )
            }

            ast::ExprKind::Field { base, field } => {
                let base = self.expr(base)?;
                let (index, ty) = self.field(&base.ty, field)?;

                (
                    hir::ExprKind::Field {
                        base: Box::new(base),
                        field: index,
                    },
                    ty,
                )
            }

            ast::ExprKind::StructLit { name, fields } => {
                return self.struct_lit(name, fields, span);
            }
        };

        Some(hir::Expr { kind, ty, span })
    }

    fn struct_lit(
        &mut self,
        name: &ast::Ident,
        fields: &[(ast::Ident, ast::Expr)],
        span: Span,
    ) -> Option<hir::Expr> {
        let Some(&id) = self.types.by_name.get(&name.name) else {
            self.error(format!("unknown struct `{}`", name.name), name.span);
            return None;
        };

        let types = self.types;
        let def = &types.structs[id.0];
        let mut values: Vec<Option<hir::Expr>> = (0..def.fields.len()).map(|_| None).collect();
        let mut ok = true;

        for (field, value) in fields {
            let value = self.expr(value);

            let Some(index) = def.field_index(&field.name) else {
                self.error(format!("{} has no field `{}`", def.name, field.name), field.span);
                ok = false;
                continue;
            };

            if values[index].is_some() {
                self.error(format!("field `{}` is given more than once", field.name), field.span);
                ok = false;
                continue;
            }

            let Some(value) = value else {
                ok = false;
                continue;
            };

            let expected = &def.fields[index].ty;
            if value.ty != *expected {
                self.error(format!("expected {expected}, found {}", value.ty), value.span);
                ok = false;
            }

            values[index] = Some(value);
        }

        let missing = def
            .fields
            .iter()
            .zip(&values)
            .filter(|(_, value)| value.is_none())
            .map(|(field, _)| format!("`{}`", field.name))
            .collect::<Vec<_>>();

        if ok && !missing.is_empty() {
            self.error(
                format!("missing field{} {}", if missing.len() == 1 { "" } else { "s" }, missing.join(", ")),
                span,
            );
            return None;
        }

        if !ok {
            return None;
        }

        Some(hir::Expr {
            kind: hir::ExprKind::StructLit(id, values.into_iter().map(Option::unwrap).collect()),
            ty: Type::Struct(id, Rc::from(def.name.as_str())),
            span,
        })
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

    const POINT: &str = "struct Point { x: i32, y: i32 }";

    #[test]
    fn unknown_type() {
        let (message, at) = error("fn f(p: Pnt): i32 { return 0 } fn main(): i32 { return 0 }");
        assert_eq!(message, "unknown type `Pnt`");
        assert_eq!(at, "Pnt");
    }

    #[test]
    fn unknown_field() {
        let source = format!(
            "{POINT} fn main(): i32 {{ p: Point p = Point(x: 1, y: 2) return p.z }}"
        );
        let (message, at) = error(&source);
        assert_eq!(message, "Point has no field `z`");
        assert_eq!(at, "z");
    }

    #[test]
    fn field_of_a_scalar() {
        let (message, _) = error("fn main(): i32 { x: i32 x = 1 return x.y }");
        assert_eq!(message, "i32 has no field `y`");
    }

    #[test]
    fn missing_struct_field() {
        let source = format!("{POINT} fn main(): i32 {{ p: Point p = Point(x: 1) return 0 }}");
        let (message, _) = error(&source);
        assert_eq!(message, "missing field `y`");
    }

    #[test]
    fn struct_field_given_twice() {
        let source = format!(
            "{POINT} fn main(): i32 {{ p: Point p = Point(x: 1, x: 2, y: 3) return 0 }}"
        );
        let (message, _) = error(&source);
        assert_eq!(message, "field `x` is given more than once");
    }

    #[test]
    fn struct_field_type() {
        let source = format!(
            "{POINT} fn main(): i32 {{ p: Point p = Point(x: true, y: 3) return 0 }}"
        );
        let (message, at) = error(&source);
        assert_eq!(message, "expected i32, found bool");
        assert_eq!(at, "true");
    }

    #[test]
    fn struct_without_named_fields() {
        let source = format!("{POINT} fn main(): i32 {{ p: Point p = Point(1, 2) return 0 }}");
        let (message, _) = error(&source);
        assert_eq!(message, "struct `Point` is built with named fields: `Point(field: value, ...)`");
    }

    #[test]
    fn recursive_struct() {
        let errors = check_source(
            "struct A { b: [B; 2] } struct B { a: A } fn main(): i32 { return 0 }",
        )
        .unwrap_err();
        // Both structs are part of the cycle.
        let messages = errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>();
        assert_eq!(
            messages,
            [
                "struct `A` contains itself, so it would have infinite size",
                "struct `B` contains itself, so it would have infinite size",
            ]
        );
    }

    #[test]
    fn struct_and_function_with_the_same_name() {
        let (message, _) = error("struct main {} fn main(): i32 { return 0 }");
        assert_eq!(message, "`main` is defined as both a struct and a function");
    }

    #[test]
    fn structs_cannot_be_compared() {
        let source = format!(
            "{POINT} fn main(): i32 {{ p: Point p = Point(x: 1, y: 2) if p == p {{ return 1 }} return 0 }}"
        );
        let (message, _) = error(&source);
        assert_eq!(message, "cannot apply `==` to Point and Point");
    }

    #[test]
    fn field_assignment_needs_assigned_struct() {
        let source = format!("{POINT} fn main(): i32 {{ p: Point p.x = 1 return 0 }}");
        let (message, at) = error(&source);
        assert_eq!(message, "`p` is used before it is assigned");
        assert_eq!(at, "p");
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
