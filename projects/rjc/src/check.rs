//! Name resolution and type checking: turns the AST into [`hir`].
//!
//! Besides resolving names and checking types, this pass enforces:
//! - every variable is assigned before it is read (definite assignment);
//! - every function with return values returns on all paths;
//! - no statement follows a `return` in the same block;
//! - no type contains itself;
//! - a `fn main(): i32` exists.
//!
//! Named types (`type NodeId i32`) are distinct from their underlying type
//! but support its operations. Integer and bool literals adapt to named
//! types, and an array value can be assigned to a named array type; any
//! other change of type needs a conversion like `NodeId(x)` or `i32(id)`.
//! The output uses underlying types only, so later stages never see named
//! types other than structs.
//!
//! Errors are collected rather than stopping at the first one. Once part of
//! the program fails to check, it is dropped, so errors don't cascade.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    diagnostic::{suggest, Code, Error, Span},
    hir::{self, BinOp, FuncId, LocalId, StructDef, Type},
    syntax::ast,
    types::{Field, NamedId, StructId},
};

pub fn check(module: &ast::Module) -> Result<hir::Program, Vec<Error>> {
    let mut errors = vec![];

    let mut functions_by_name: HashMap<&str, FuncId> = HashMap::new();
    for (index, function) in module.functions.iter().enumerate() {
        if function.receiver.is_some() {
            continue;
        }

        let name = function.name.name.as_str();

        if functions_by_name.contains_key(name) {
            errors.push(Error::new(Code::Duplicate, 
                format!("function `{name}` is already defined"),
                function.name.span,
            ));
        } else {
            functions_by_name.insert(name, FuncId(index));
        }
    }

    let types = Types::new(module, &functions_by_name, &mut errors);

    // Methods, by receiver type and name.
    let mut methods: HashMap<(Type, String), FuncId> = HashMap::new();
    let mut receivers = vec![None; module.functions.len()];

    for (index, function) in module.functions.iter().enumerate() {
        let Some(receiver) = &function.receiver else {
            continue;
        };
        let name = &function.name.name;

        let ty = match types.by_name.get(&receiver.name) {
            Some(ty) => ty.clone(),
            None if matches!(receiver.name.as_str(), "i32" | "bool") => {
                errors.push(Error::new(Code::MethodOnBuiltin, 
                    "methods can only be defined on types declared with `type`",
                    receiver.span,
                ));
                continue;
            }
            None => {
                errors.push(Error::new(Code::UnknownType, format!("unknown type `{}`", receiver.name), receiver.span));
                continue;
            }
        };

        if let Type::Struct(id, _) = &ty {
            if types.structs[id.0].field_index(name).is_some() {
                errors.push(Error::new(Code::Duplicate, 
                    format!("{ty} has both a field and a method called `{name}`"),
                    function.name.span,
                ));
                continue;
            }
        }

        let key = (ty.clone(), name.clone());
        if methods.contains_key(&key) {
            errors.push(Error::new(Code::Duplicate, 
                format!("method `{ty}.{name}` is already defined"),
                function.name.span,
            ));
            continue;
        }

        methods.insert(key, FuncId(index));
        receivers[index] = Some(ty);
    }

    // A signature is `None` if one of its types failed to resolve; calls to
    // it are then skipped rather than reported again.
    let signatures = module
        .functions
        .iter()
        .zip(&receivers)
        .map(|(function, receiver)| {
            if function.receiver.is_some() && receiver.is_none() {
                return None;
            }

            let params = receiver
                .iter()
                .map(|ty| Some(ty.clone()))
                .chain(function.params.iter().map(|param| types.resolve(&param.ty, &mut errors)))
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
                errors.push(Error::new(Code::Main, 
                    "`main` must be declared as `fn main(): i32`",
                    module.functions[id.0].name.span,
                ));
            }
        }
        None => errors.push(Error::new(Code::Main, "no `main` function defined", Span::new(0, 0))),
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
                methods: &methods,
                errors: &mut errors,
                returns: &signature.returns,
                locals: vec![],
                scopes: vec![],
                assigned: HashSet::new(),
            }
            .function(function, signature)
        })
        .collect::<Vec<_>>();

    let examples = module
        .examples
        .iter()
        .map(|example| {
            FunctionChecker {
                types: &types,
                signatures: &signatures,
                functions_by_name: &functions_by_name,
                methods: &methods,
                errors: &mut errors,
                returns: &[],
                locals: vec![],
                scopes: vec![HashMap::new()],
                assigned: HashSet::new(),
            }
            .example(example)
        })
        .collect::<Vec<_>>();

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut functions = functions.into_iter().map(Option::unwrap).collect::<Vec<_>>();
    for function in &mut functions {
        types.erase_function(function);
    }

    let mut examples = examples.into_iter().map(Option::unwrap).collect::<Vec<_>>();
    for example in &mut examples {
        types.erase_function(&mut example.function);
    }

    let structs = types
        .structs
        .iter()
        .map(|def| StructDef {
            name: def.name.clone(),
            fields: def
                .fields
                .iter()
                .map(|field| Field {
                    name: field.name.clone(),
                    ty: types.erase(&field.ty),
                })
                .collect(),
        })
        .collect();

    Ok(hir::Program {
        structs,
        functions,
        main: main.unwrap(),
        examples,
    })
}

struct Signature {
    params: Vec<Type>,
    returns: Vec<Type>,
}

struct NamedDef {
    underlying: Type,
}

/// The program's named types, and resolution of type syntax.
struct Types {
    structs: Vec<StructDef>,
    named: Vec<NamedDef>,
    /// `Type::Struct` or `Type::Named`.
    by_name: HashMap<String, Type>,
}

impl Types {
    fn new(
        module: &ast::Module,
        functions_by_name: &HashMap<&str, FuncId>,
        errors: &mut Vec<Error>,
    ) -> Types {
        let mut types = Types {
            structs: vec![],
            named: vec![],
            by_name: HashMap::new(),
        };

        // Register every name first, so definitions can refer to any type.
        let mut defined = vec![];
        for def in &module.types {
            let name = &def.name.name;

            if types.by_name.contains_key(name) || matches!(name.as_str(), "i32" | "bool") {
                errors.push(Error::new(Code::Duplicate, 
                    format!("type `{name}` is already defined"),
                    def.name.span,
                ));
                continue;
            }
            if functions_by_name.contains_key(name.as_str()) {
                // `Name(...)` must mean one thing.
                errors.push(Error::new(Code::Duplicate, 
                    format!("`{name}` is defined as both a type and a function"),
                    def.name.span,
                ));
                continue;
            }

            let ty = match def.body {
                ast::TypeDefBody::Struct(_) => {
                    types.structs.push(StructDef {
                        name: name.clone(),
                        fields: vec![],
                    });
                    Type::Struct(StructId(types.structs.len() - 1), Rc::from(name.as_str()))
                }
                ast::TypeDefBody::Type(_) => {
                    types.named.push(NamedDef {
                        underlying: Type::I32,
                    });
                    Type::Named(NamedId(types.named.len() - 1), Rc::from(name.as_str()))
                }
            };

            types.by_name.insert(name.clone(), ty.clone());
            defined.push((def, ty));
        }

        for (def, ty) in &defined {
            match (&def.body, ty) {
                (ast::TypeDefBody::Struct(decls), Type::Struct(id, _)) => {
                    let mut fields: Vec<Field> = vec![];

                    for field in decls {
                        if fields.iter().any(|f| f.name == field.name.name) {
                            errors.push(Error::new(Code::Duplicate, 
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

                    types.structs[id.0].fields = fields;
                }
                (ast::TypeDefBody::Type(expr), Type::Named(id, _)) => {
                    if let Some(underlying) = types.resolve(expr, errors) {
                        types.named[id.0].underlying = underlying;
                    }
                }
                _ => unreachable!(),
            }
        }

        let mut cyclic = vec![];
        for (def, ty) in &defined {
            if types.contains(&mut vec![], ty, ty) {
                errors.push(Error::new(Code::InfiniteType, 
                    format!(
                        "type `{}` contains itself, so it would have infinite size",
                        def.name.name
                    ),
                    def.name.span,
                ));
                cyclic.push(ty.clone());
            }
        }

        // Break cycles so later lookups through them terminate.
        for ty in cyclic {
            match ty {
                Type::Named(id, _) => types.named[id.0].underlying = Type::I32,
                Type::Struct(id, _) => types.structs[id.0].fields.clear(),
                _ => {}
            }
        }

        types
    }

    /// Whether `outer` stores a `target` somewhere inside it.
    fn contains(&self, visited: &mut Vec<Type>, outer: &Type, target: &Type) -> bool {
        let inner = match outer {
            Type::I32 | Type::Bool => return false,
            Type::Array(elem, _) => vec![(**elem).clone()],
            Type::Struct(id, _) => self.structs[id.0].fields.iter().map(|f| f.ty.clone()).collect(),
            Type::Named(id, _) => vec![self.named[id.0].underlying.clone()],
        };

        inner.iter().any(|ty| {
            if ty == target {
                return true;
            }
            if visited.contains(ty) {
                return false;
            }
            visited.push(ty.clone());
            self.contains(visited, ty, target)
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
                Some(ty) => Some(ty.clone()),
                None => {
                    let mut error =
                        Error::new(Code::UnknownType, format!("unknown type `{}`", name.name), name.span);
                    let names = self.by_name.keys().map(String::as_str).chain(["i32", "bool"]);
                    if let Some(suggestion) = suggest(&name.name, names) {
                        error.help(format!("did you mean `{suggestion}`?"));
                    }
                    errors.push(error);
                    None
                }
            },
        }
    }

    /// The type with named non-struct types replaced by what they name, at
    /// the top level only: `[NodeId; 2]` stays as it is.
    fn underlying(&self, ty: &Type) -> Type {
        let mut ty = ty.clone();
        while let Type::Named(id, _) = ty {
            ty = self.named[id.0].underlying.clone();
        }
        ty
    }

    /// The type with every named non-struct type replaced, as later stages
    /// see it.
    fn erase(&self, ty: &Type) -> Type {
        match ty {
            Type::I32 | Type::Bool | Type::Struct(..) => ty.clone(),
            Type::Array(elem, len) => Type::Array(Box::new(self.erase(elem)), *len),
            Type::Named(..) => self.erase(&self.underlying(ty)),
        }
    }

    /// Whether values convert between `a` and `b`: they have the same
    /// underlying type, or are structs with the same fields.
    fn convertible(&self, a: &Type, b: &Type) -> bool {
        let (a, b) = (self.underlying(a), self.underlying(b));

        if a == b {
            return true;
        }

        match (&a, &b) {
            (Type::Struct(x, _), Type::Struct(y, _)) => {
                let (x, y) = (&self.structs[x.0].fields, &self.structs[y.0].fields);
                x.len() == y.len()
                    && x.iter().zip(y).all(|(f, g)| f.name == g.name && f.ty == g.ty)
            }
            _ => false,
        }
    }

    fn erase_function(&self, function: &mut hir::Function) {
        for local in &mut function.locals {
            local.ty = self.erase(&local.ty);
        }
        for ty in &mut function.returns {
            *ty = self.erase(ty);
        }
        self.erase_block(&mut function.body);
    }

    fn erase_block(&self, block: &mut hir::Block) {
        for stmt in &mut block.stmts {
            match &mut stmt.kind {
                hir::StmtKind::Assign { targets, values } => {
                    targets.iter_mut().for_each(|t| self.erase_place(t));
                    values.iter_mut().for_each(|v| self.erase_value(v));
                }
                hir::StmtKind::Call(call) => self.erase_call(call),
                hir::StmtKind::If {
                    cond,
                    then_block,
                    else_block,
                } => {
                    self.erase_expr(cond);
                    self.erase_block(then_block);
                    self.erase_block(else_block);
                }
                hir::StmtKind::While { cond, body } => {
                    self.erase_expr(cond);
                    self.erase_block(body);
                }
                hir::StmtKind::Return(values) => {
                    values.iter_mut().for_each(|v| self.erase_value(v))
                }
            }
        }
    }

    fn erase_place(&self, place: &mut hir::Place) {
        for elem in &mut place.path {
            if let hir::PlaceElem::Index(index) = elem {
                self.erase_expr(index);
            }
        }
    }

    fn erase_value(&self, value: &mut hir::Value) {
        match value {
            hir::Value::Single(expr) => self.erase_expr(expr),
            hir::Value::Multi(call) => self.erase_call(call),
        }
    }

    fn erase_call(&self, call: &mut hir::Call) {
        match &mut call.receiver {
            Some(hir::Receiver::Place(place)) => self.erase_place(place),
            Some(hir::Receiver::Value(expr)) => self.erase_expr(expr),
            None => {}
        }
        call.args.iter_mut().for_each(|v| self.erase_value(v));
    }

    fn erase_expr(&self, expr: &mut hir::Expr) {
        expr.ty = self.erase(&expr.ty);

        match &mut expr.kind {
            hir::ExprKind::Int(_) | hir::ExprKind::Bool(_) | hir::ExprKind::Local(_) => {}
            hir::ExprKind::Call(call) => self.erase_call(call),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.erase_expr(lhs);
                self.erase_expr(rhs);
            }
            hir::ExprKind::ArrayList(elems) | hir::ExprKind::StructLit(_, elems) => {
                elems.iter_mut().for_each(|e| self.erase_expr(e))
            }
            hir::ExprKind::ArrayRepeat(value, _) => self.erase_expr(value),
            hir::ExprKind::Index { base, index } => {
                self.erase_expr(base);
                self.erase_expr(index);
            }
            hir::ExprKind::Field { base, .. } => self.erase_expr(base),
            hir::ExprKind::Convert(inner) => self.erase_expr(inner),
        }
    }
}

/// A checked call and the types it returns.
type CallResult = Option<(hir::Call, Vec<Type>)>;

/// A value's type, where it is, and whether it is made only of literals
/// (which adapt to named types).
type Found = (Type, Span, bool);

fn is_literal(expr: &hir::Expr) -> bool {
    match &expr.kind {
        hir::ExprKind::Int(_) | hir::ExprKind::Bool(_) => true,
        hir::ExprKind::ArrayList(elems) => elems.iter().all(is_literal),
        hir::ExprKind::ArrayRepeat(value, _) => is_literal(value),
        _ => false,
    }
}

struct FunctionChecker<'a> {
    types: &'a Types,
    signatures: &'a [Option<Signature>],
    functions_by_name: &'a HashMap<&'a str, FuncId>,
    methods: &'a HashMap<(Type, String), FuncId>,
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
    fn error(&mut self, code: Code, message: impl Into<String>, span: Span) -> &mut Error {
        self.errors.push(Error::new(code, message, span));
        self.errors.last_mut().unwrap()
    }

    fn under(&self, ty: &Type) -> Type {
        self.types.underlying(ty)
    }

    /// Whether a value of type `found` can be stored where `expected` is
    /// needed without a conversion.
    fn assignable(&self, found: &Type, literal: bool, expected: &Type) -> bool {
        found == expected
            || (literal && self.adapts(found, expected))
            || ((matches!(found, Type::Array(..)) || matches!(expected, Type::Array(..)))
                && self.under(found) == self.under(expected))
    }

    /// Whether a literal of type `found` (`1`, `[1, 2]`, `[0; 3]`) can be
    /// read as a value of type `expected`.
    fn adapts(&self, found: &Type, expected: &Type) -> bool {
        if found == expected {
            return true;
        }

        match (found, self.under(expected)) {
            (Type::Array(found, n), Type::Array(expected, m)) => {
                n == &m && self.adapts(found, &expected)
            }
            (found, expected) => *found == expected,
        }
    }

    fn function(mut self, function: &ast::Function, signature: &Signature) -> Option<hir::Function> {
        // Parameters and the top-level statements of the body share a scope,
        // so a body declaration can't shadow a parameter.
        self.scopes.push(HashMap::new());

        let mut decls = vec![];
        if let Some(receiver) = &function.receiver {
            decls.push(ast::VarDecl {
                name: ast::Ident {
                    name: "this".to_string(),
                    span: receiver.span,
                },
                ty: ast::TypeExpr {
                    kind: ast::TypeExprKind::Named(receiver.clone()),
                    span: receiver.span,
                },
                span: receiver.span,
            });
        }
        decls.extend(function.params.iter().cloned());

        let mut params = vec![];
        for (param, ty) in decls.iter().zip(&signature.params) {
            if let Some(id) = self.declare_typed(param, ty.clone()) {
                self.assigned.insert(id);
                params.push(id);
            }
        }

        let (body, diverges) = self.stmts(&function.body)?;

        if !diverges && !function.returns.is_empty() {
            self.error(Code::MissingReturn, 
                format!(
                    "function `{}` can reach its end without returning a value",
                    function.name.name
                ),
                function.name.span,
            )
            .help("add a `return` at the end; loops never count as returning");
            return None;
        }

        if params.len() != decls.len() {
            return None;
        }

        let name = match &function.receiver {
            Some(receiver) => format!("{}.{}", receiver.name, function.name.name),
            None => function.name.name.clone(),
        };

        Some(hir::Function {
            name,
            is_method: function.receiver.is_some(),
            params,
            returns: signature.returns.clone(),
            locals: self.locals,
            body,
            span: function.span,
        })
    }

    /// Checks an `example` as a function returning both sides of a
    /// comparison, or a single bool.
    fn example(mut self, example: &ast::Example) -> Option<hir::Example> {
        let span = example.span;

        let (values, returns, compare) = match &example.expr.kind {
            // Examples may compare any two values, including arrays and
            // structs.
            ast::ExprKind::Binary {
                op: op @ (BinOp::Eq | BinOp::Ne),
                lhs,
                rhs,
            } => {
                let lhs = self.expr(lhs);
                let rhs = self.expr(rhs);
                let (lhs, rhs) = (lhs?, rhs?);

                let comparable = lhs.ty == rhs.ty
                    || (is_literal(&rhs) && self.adapts(&rhs.ty, &lhs.ty))
                    || (is_literal(&lhs) && self.adapts(&lhs.ty, &rhs.ty));

                if !comparable {
                    self.error(
                        Code::Mismatch,
                        format!("cannot compare {} with {}", lhs.ty, rhs.ty),
                        example.expr.span,
                    )
                    .types(&lhs.ty, &rhs.ty);
                    return None;
                }

                let types = vec![lhs.ty.clone(), rhs.ty.clone()];
                (vec![hir::Value::Single(lhs), hir::Value::Single(rhs)], types, Some(*op))
            }

            _ => {
                let expr = self.expr(&example.expr)?;

                if self.under(&expr.ty) != Type::Bool {
                    self.error(
                        Code::NotBool,
                        format!("an example must be a comparison or a bool, found {}", expr.ty),
                        expr.span,
                    )
                    .help("compare the result with what you expect: `f(x) == y`");
                    return None;
                }

                (vec![hir::Value::Single(expr)], vec![Type::Bool], None)
            }
        };

        let function = hir::Function {
            name: "example".to_string(),
            is_method: false,
            params: vec![],
            returns,
            locals: self.locals,
            body: hir::Block {
                stmts: vec![hir::Stmt {
                    kind: hir::StmtKind::Return(values),
                    span,
                }],
            },
            span,
        };

        Some(hir::Example {
            function,
            compare,
            span,
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
            self.error(Code::Duplicate, 
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

        if name == "this" {
            self.error(Code::ThisOutsideMethod, "`this` can only be used in methods", ident.span);
        } else if self.functions_by_name.contains_key(name) {
            self.error(Code::WrongKindOfName, format!("`{name}` is a function, not a variable"), ident.span);
        } else {
            let locals = self
                .scopes
                .iter()
                .flat_map(|scope| scope.keys().map(String::as_str))
                .collect::<Vec<_>>();
            let suggestion = suggest(name, locals).map(str::to_string);
            let error = self.error(Code::UnknownVariable, format!("unknown variable `{name}`"), ident.span);
            if let Some(suggestion) = suggestion {
                error.help(format!("did you mean `{suggestion}`?"));
            }
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
                self.error(Code::Unreachable, "unreachable statement after `return`", stmt.span)
                    .help("remove it, or move it before the `return`");
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
                                self.error(Code::AssignedTwice, 
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
                        self.error(Code::AssignedTwice, 
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

            ast::StmtKind::Call(call) => {
                let (call, _) = self.call(call, span)?;
                (hir::StmtKind::Call(call), false)
            }

            ast::StmtKind::Expr(ast::Expr {
                kind:
                    ast::ExprKind::MethodCall {
                        receiver,
                        method,
                        args,
                    },
                ..
            }) => {
                let (call, _) = self.method_call(receiver, method, args, span)?;
                (hir::StmtKind::Call(call), false)
            }

            ast::StmtKind::Expr(_) => unreachable!("the parser only allows method calls"),

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
            self.error(Code::UseBeforeAssign, 
                format!("`{}` is used before it is assigned", target.name.name),
                target.name.span,
            )
            .help("assigning a part keeps the rest, so assign the whole value first");
            return None;
        }

        let mut path = vec![];

        for elem in &target.path {
            match elem {
                ast::PlaceElem::Index(index) => {
                    let Type::Array(elem, len) = self.under(&ty) else {
                        self.error(Code::NotIndexable, format!("cannot index into a value of type {ty}"), target.span);
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
        let Type::Struct(id, _) = self.under(ty) else {
            self.error(Code::UnknownField, format!("{ty} has no field `{}`", name.name), name.span);
            return None;
        };

        let def = &self.types.structs[id.0];

        let Some(index) = def.field_index(&name.name) else {
            let suggestion =
                suggest(&name.name, def.fields.iter().map(|f| f.name.as_str())).map(str::to_string);
            let error = self.error(Code::UnknownField, format!("{ty} has no field `{}`", name.name), name.span);
            if let Some(suggestion) = suggestion {
                error.help(format!("did you mean `{suggestion}`?"));
            }
            return None;
        };

        Some((index, def.fields[index].ty.clone()))
    }

    fn condition(&mut self, cond: &ast::Expr) -> Option<hir::Expr> {
        let cond = self.expr(cond)?;

        if self.under(&cond.ty) != Type::Bool {
            self.error(Code::NotBool, 
                format!("condition must be bool, found {}", cond.ty),
                cond.span,
            )
            .types(Type::Bool, &cond.ty);
            return None;
        }

        Some(cond)
    }

    /// Checks a call or method call, if `expr` is one. Calls to types
    /// (conversions and `Empty()`) are not calls here.
    fn any_call(&mut self, expr: &ast::Expr) -> Option<CallResult> {
        match &expr.kind {
            ast::ExprKind::Call(call)
                if self.functions_by_name.contains_key(call.callee.name.as_str())
                    || !self.is_type_name(&call.callee.name) =>
            {
                Some(self.call(call, expr.span))
            }
            ast::ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => Some(self.method_call(receiver, method, args, expr.span)),
            _ => None,
        }
    }

    fn is_type_name(&self, name: &str) -> bool {
        self.types.by_name.contains_key(name) || matches!(name, "i32" | "bool")
    }

    /// Checks a value list. Calls that don't return exactly one value
    /// contribute all their return values. Returns the values along with the
    /// type and source span of each individual value.
    fn values(&mut self, items: &[ast::Expr]) -> Option<(Vec<hir::Value>, Vec<Found>)> {
        let mut values = vec![];
        let mut types = vec![];
        let mut ok = true;

        for item in items {
            if let Some(result) = self.any_call(item) {
                match result {
                    Some((call, returns)) if returns.len() == 1 => {
                        let ty = returns.into_iter().next().unwrap();
                        types.push((ty.clone(), item.span, false));
                        values.push(hir::Value::Single(hir::Expr {
                            kind: hir::ExprKind::Call(call),
                            ty,
                            span: item.span,
                        }));
                    }
                    Some((call, returns)) => {
                        types.extend(returns.into_iter().map(|ty| (ty, item.span, false)));
                        values.push(hir::Value::Multi(call));
                    }
                    None => ok = false,
                }
                continue;
            }

            match self.expr(item) {
                Some(expr) => {
                    types.push((expr.ty.clone(), expr.span, is_literal(&expr)));
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
        found: &[Found],
        span: Span,
        what: &str,
        noun: &str,
    ) -> Option<()> {
        if expected.len() != found.len() {
            let plural = if expected.len() == 1 { "" } else { "s" };

            self.error(Code::Count, 
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

        for (expected, (found, span, literal)) in expected.iter().zip(found) {
            if !self.assignable(found, *literal, expected) {
                let convertible = self.types.convertible(found, expected);
                let error = self.error(Code::Mismatch, format!("expected {expected}, found {found}"), *span);
                error.types(expected, found);
                if convertible {
                    error.help(format!("convert it: `{expected}(...)`"));
                }
                ok = false;
            }
        }

        ok.then_some(())
    }

    fn call(&mut self, call: &ast::Call, span: Span) -> CallResult {
        let name = call.callee.name.as_str();

        let Some(&func) = self.functions_by_name.get(name) else {
            if self.find_local(name).is_some() {
                self.error(Code::WrongKindOfName, format!("`{name}` is a variable, not a function"), call.callee.span);
            } else if self.is_type_name(name) {
                self.error(Code::WrongKindOfName, 
                    format!("`{name}` is a type; it can't be called here"),
                    call.callee.span,
                );
            } else {
                let suggestion =
                    suggest(name, self.functions_by_name.keys().copied()).map(str::to_string);
                let error =
                    self.error(Code::UnknownFunction, format!("unknown function `{name}`"), call.callee.span);
                if let Some(suggestion) = suggestion {
                    error.help(format!("did you mean `{suggestion}`?"));
                }
            }

            return None;
        };

        let (args, types) = self.values(&call.args)?;

        // The signature didn't resolve; that was already reported.
        let signatures = self.signatures;
        let signature = signatures[func.0].as_ref()?;

        self.expect_values(&signature.params, &types, span, &format!("`{name}`"), "argument")?;

        let call = hir::Call {
            func,
            receiver: None,
            args,
            span,
        };
        Some((call, signature.returns.clone()))
    }

    fn method_call(
        &mut self,
        receiver: &ast::Expr,
        method: &ast::Ident,
        args: &[ast::Expr],
        span: Span,
    ) -> CallResult {
        let receiver = self.expr(receiver)?;
        let key = (receiver.ty.clone(), method.name.clone());

        let Some(&func) = self.methods.get(&key) else {
            let methods = self
                .methods
                .keys()
                .filter(|(ty, _)| *ty == receiver.ty)
                .map(|(_, name)| name.as_str());
            let suggestion = suggest(&method.name, methods).map(str::to_string);
            let error = self.error(
                Code::UnknownMethod,
                format!("{} has no method `{}`", receiver.ty, method.name),
                method.span,
            );
            if let Some(suggestion) = suggestion {
                error.help(format!("did you mean `{suggestion}`?"));
            }
            return None;
        };

        let (args, types) = self.values(args)?;

        let signatures = self.signatures;
        let signature = signatures[func.0].as_ref()?;
        let name = format!("`{}.{}`", receiver.ty, method.name);

        self.expect_values(&signature.params[1..], &types, span, &name, "argument")?;

        let call = hir::Call {
            func,
            receiver: Some(into_receiver(receiver)),
            args,
            span,
        };
        Some((call, signature.returns.clone()))
    }

    fn expr(&mut self, expr: &ast::Expr) -> Option<hir::Expr> {
        let span = expr.span;

        if let Some(result) = self.any_call(expr) {
            let (call, returns) = result?;

            let [ty] = &returns[..] else {
                let name = match &expr.kind {
                    ast::ExprKind::Call(call) => call.callee.name.clone(),
                    ast::ExprKind::MethodCall { method, .. } => method.name.clone(),
                    _ => unreachable!(),
                };
                self.error(Code::MultipleValues, 
                    format!(
                        "`{name}` returns {} values, but one value is expected here",
                        returns.len()
                    ),
                    span,
                )
                .help("assign the results to variables first: `a, b = f(...)`");
                return None;
            };

            return Some(hir::Expr {
                kind: hir::ExprKind::Call(call),
                ty: ty.clone(),
                span,
            });
        }

        let (kind, ty) = match &expr.kind {
            ast::ExprKind::Int(value) => (hir::ExprKind::Int(*value), Type::I32),

            ast::ExprKind::Bool(value) => (hir::ExprKind::Bool(*value), Type::Bool),

            ast::ExprKind::Var(ident) => {
                let id = self.lookup(ident)?;

                if !self.assigned.contains(&id) {
                    self.error(Code::UseBeforeAssign, 
                        format!("`{}` is used before it is assigned", ident.name),
                        ident.span,
                    )
                    .help(format!(
                        "assign `{}` on every path before this; assignments inside a loop \
                         don't count after it",
                        ident.name
                    ));
                    return None;
                }

                (hir::ExprKind::Local(id), self.locals[id.0].ty.clone())
            }

            // A call to a type: `Empty()`, or a conversion like `NodeId(x)`.
            ast::ExprKind::Call(call) => return self.type_call(call, span),

            ast::ExprKind::MethodCall { .. } => unreachable!("handled as a call"),

            ast::ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.expr(lhs);
                let rhs = self.expr(rhs);
                let (lhs, rhs) = (lhs?, rhs?);

                // A literal adapts to a named type on the other side.
                let common = if lhs.ty == rhs.ty {
                    Some(lhs.ty.clone())
                } else if is_literal(&rhs) && self.adapts(&rhs.ty, &lhs.ty) {
                    Some(lhs.ty.clone())
                } else if is_literal(&lhs) && self.adapts(&lhs.ty, &rhs.ty) {
                    Some(rhs.ty.clone())
                } else {
                    None
                };

                let ty = match (op, common.as_ref().map(|ty| (ty, self.under(ty)))) {
                    (BinOp::Add | BinOp::Sub, Some((ty, Type::I32))) => ty.clone(),
                    (BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, Some((_, Type::I32))) => {
                        Type::Bool
                    }
                    (BinOp::Eq | BinOp::Ne, Some((_, Type::I32 | Type::Bool))) => Type::Bool,

                    (op, _) => {
                        self.error(Code::BadOperands, 
                            format!("cannot apply `{}` to {} and {}", op.as_str(), lhs.ty, rhs.ty),
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
                    self.error(Code::EmptyArray, 
                        "the element type of an empty array can't be inferred; \
                         use a repeat like `[0; 0]`",
                        span,
                    );
                    return None;
                };

                let elem = first.ty.clone();

                for other in &checked[1..] {
                    if other.ty != elem {
                        self.error(Code::Mismatch, format!("expected {elem}, found {}", other.ty), other.span)
                            .types(&elem, &other.ty);
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

                let Type::Array(elem, len) = self.under(&base.ty) else {
                    self.error(Code::NotIndexable, format!("cannot index into a value of type {}", base.ty), base.span);
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

    /// `Name(...)` where `Name` is a type: `Empty()` for a struct without
    /// fields, or a conversion `Name(value)`.
    fn type_call(&mut self, call: &ast::Call, span: Span) -> Option<hir::Expr> {
        let name = &call.callee.name;

        let target = match name.as_str() {
            "i32" => Type::I32,
            "bool" => Type::Bool,
            _ => self.types.by_name[name].clone(),
        };

        if call.args.is_empty() && matches!(target, Type::Struct(..)) {
            return self.struct_lit(&call.callee, &[], span);
        }

        let [arg] = &call.args[..] else {
            if matches!(target, Type::Struct(..)) {
                self.error(Code::StructFields, 
                    format!("struct `{name}` is built with named fields: `{name}(field: value, ...)`"),
                    call.callee.span,
                );
            } else {
                self.error(Code::BadConversion, format!("a conversion to `{name}` takes one value"), span);
            }
            return None;
        };

        let value = self.expr(arg)?;

        if !self.types.convertible(&value.ty, &target) {
            self.error(Code::BadConversion, format!("cannot convert {} to {target}", value.ty), value.span);
            return None;
        }

        Some(hir::Expr {
            kind: hir::ExprKind::Convert(Box::new(value)),
            ty: target,
            span,
        })
    }

    fn struct_lit(
        &mut self,
        name: &ast::Ident,
        fields: &[(ast::Ident, ast::Expr)],
        span: Span,
    ) -> Option<hir::Expr> {
        let Some(Type::Struct(id, _)) = self.types.by_name.get(&name.name).cloned() else {
            self.error(Code::UnknownType, format!("unknown struct `{}`", name.name), name.span);
            return None;
        };

        let types = self.types;
        let def = &types.structs[id.0];
        let mut values: Vec<Option<hir::Expr>> = (0..def.fields.len()).map(|_| None).collect();
        let mut ok = true;

        for (field, value) in fields {
            let value = self.expr(value);

            let Some(index) = def.field_index(&field.name) else {
                self.error(Code::UnknownField, format!("{} has no field `{}`", def.name, field.name), field.span);
                ok = false;
                continue;
            };

            if values[index].is_some() {
                self.error(Code::StructFields, format!("field `{}` is given more than once", field.name), field.span);
                ok = false;
                continue;
            }

            let Some(value) = value else {
                ok = false;
                continue;
            };

            let expected = &def.fields[index].ty;
            if !self.assignable(&value.ty, is_literal(&value), expected) {
                self.error(Code::Mismatch, format!("expected {expected}, found {}", value.ty), value.span)
                    .types(expected, &value.ty);
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
            self.error(Code::StructFields, 
                format!(
                    "missing field{} {}",
                    if missing.len() == 1 { "" } else { "s" },
                    missing.join(", ")
                ),
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

        if self.under(&index.ty) != Type::I32 {
            self.error(Code::BadIndexType, format!("array index must be i32, found {}", index.ty), index.span);
            return None;
        }

        if let hir::ExprKind::Int(value) = index.kind {
            if value < 0 || value as u32 >= len {
                self.error(Code::IndexOutOfBounds, 
                    format!("index {value} is out of bounds for an array of length {len}"),
                    index.span,
                );
                return None;
            }
        }

        Some(index)
    }
}

/// A method receiver: a place if `expr` names a variable or a part of one,
/// otherwise a plain value.
fn into_receiver(expr: hir::Expr) -> hir::Receiver {
    fn is_place(expr: &hir::Expr) -> bool {
        match &expr.kind {
            hir::ExprKind::Local(_) => true,
            hir::ExprKind::Index { base, .. } | hir::ExprKind::Field { base, .. } => is_place(base),
            _ => false,
        }
    }

    fn into_place(expr: hir::Expr) -> hir::Place {
        match expr.kind {
            hir::ExprKind::Local(local) => hir::Place {
                local,
                path: vec![],
            },
            hir::ExprKind::Index { base, index } => {
                let mut place = into_place(*base);
                place.path.push(hir::PlaceElem::Index(*index));
                place
            }
            hir::ExprKind::Field { base, field } => {
                let mut place = into_place(*base);
                place.path.push(hir::PlaceElem::Field(field));
                place
            }
            _ => unreachable!(),
        }
    }

    if is_place(&expr) {
        hir::Receiver::Place(into_place(expr))
    } else {
        hir::Receiver::Value(Box::new(expr))
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

    const POINT: &str = "type Point struct { x: i32, y: i32 }";

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
            "type A struct { b: [B; 2] } type B struct { a: A } fn main(): i32 { return 0 }",
        )
        .unwrap_err();
        // Both structs are part of the cycle.
        let messages = errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>();
        assert_eq!(
            messages,
            [
                "type `A` contains itself, so it would have infinite size",
                "type `B` contains itself, so it would have infinite size",
            ]
        );
    }

    #[test]
    fn struct_and_function_with_the_same_name() {
        let (message, _) = error("type main struct {} fn main(): i32 { return 0 }");
        assert_eq!(message, "`main` is defined as both a type and a function");
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

    const NODE_ID: &str = "type NodeId i32";

    #[test]
    fn named_types_are_distinct() {
        let source = format!(
            "{NODE_ID} fn main(): i32 {{ id: NodeId i: i32 i = 1 id = i return 0 }}"
        );
        let (message, at) = error(&source);
        assert_eq!(message, "expected NodeId, found i32");
        assert_eq!(at, "i");
    }

    #[test]
    fn literals_adapt_to_named_types() {
        check_source(&format!(
            "{NODE_ID}
             type Ids [NodeId; 2]
             fn main(): i32 {{
               id: NodeId
               ids: Ids
               id = 1
               id = id + 1
               ids = [0, 1]
               ids = [0; 2]
               if id == 2 {{ return 0 }}
               return i32(id)
             }}"
        ))
        .unwrap();
    }

    #[test]
    fn mixing_named_and_plain_operands() {
        let source = format!(
            "{NODE_ID} fn main(): i32 {{ id: NodeId i: i32 id, i = 1, 1 return i32(id + i) }}"
        );
        let (message, _) = error(&source);
        assert_eq!(message, "cannot apply `+` to NodeId and i32");
    }

    #[test]
    fn conversion_between_unrelated_types() {
        let source = format!("{NODE_ID} fn main(): i32 {{ return i32(NodeId(true)) }}");
        let (message, at) = error(&source);
        assert_eq!(message, "cannot convert bool to NodeId");
        assert_eq!(at, "true");
    }

    #[test]
    fn struct_conversion_needs_same_fields() {
        check_source(
            "type A struct { x: i32 }
             type B struct { x: i32 }
             fn main(): i32 { a: A a = A(x: 1) b: B b = B(a) return b.x - 1 }",
        )
        .unwrap();

        let (message, _) = error(
            "type A struct { x: i32 }
             type C struct { y: i32 }
             fn main(): i32 { a: A a = A(x: 1) c: C c = C(a) return 0 }",
        );
        assert_eq!(message, "cannot convert A to C");
    }

    #[test]
    fn unknown_method() {
        let source = format!(
            "{POINT} fn main(): i32 {{ p: Point p = Point(x: 1, y: 2) return p.len() }}"
        );
        let (message, at) = error(&source);
        assert_eq!(message, "Point has no method `len`");
        assert_eq!(at, "len");
    }

    #[test]
    fn this_outside_a_method() {
        let (message, _) = error("fn main(): i32 { return this }");
        assert_eq!(message, "`this` can only be used in methods");
    }

    #[test]
    fn methods_on_builtin_types() {
        let (message, _) = error("fn i32.double(): i32 { return this } fn main(): i32 { return 0 }");
        assert_eq!(message, "methods can only be defined on types declared with `type`");
    }

    #[test]
    fn field_and_method_with_the_same_name() {
        let source = format!("{POINT} fn Point.x(): i32 {{ return 0 }} fn main(): i32 {{ return 0 }}");
        let (message, _) = error(&source);
        assert_eq!(message, "Point has both a field and a method called `x`");
    }

    #[test]
    fn duplicate_method() {
        let source = format!(
            "{POINT} fn Point.m() {{}} fn Point.m() {{}} fn main(): i32 {{ return 0 }}"
        );
        let (message, _) = error(&source);
        assert_eq!(message, "method `Point.m` is already defined");
    }

    #[test]
    fn method_argument_count() {
        let source = format!(
            "{POINT} fn Point.shift(d: i32) {{ this.x = this.x + d }}
             fn main(): i32 {{ p: Point p = Point(x: 1, y: 2) p.shift() return 0 }}"
        );
        let (message, _) = error(&source);
        assert_eq!(message, "`Point.shift` expects 1 argument, found 0");
    }

    #[test]
    fn method_and_function_with_the_same_name() {
        check_source(&format!(
            "{POINT} fn len(): i32 {{ return 0 }}
             fn Point.len(): i32 {{ return this.x }}
             fn main(): i32 {{ p: Point p = Point(x: 0, y: 0) return len() + p.len() }}"
        ))
        .unwrap();
    }

    #[test]
    fn named_type_containing_itself() {
        let errors = check_source("type A [A; 2] fn main(): i32 { return 0 }").unwrap_err();
        assert_eq!(errors[0].message, "type `A` contains itself, so it would have infinite size");
    }

    /// The code, help and expected/found types of the single error in
    /// `source`.
    fn diagnostic(source: &str) -> (Code, Option<String>, Option<(String, String)>) {
        let errors = check_source(source).expect_err("expected an error");
        assert_eq!(errors.len(), 1, "expected one error, got {errors:#?}");
        let e = &errors[0];
        let types = e.expected.clone().zip(e.found.clone());
        (e.code, e.help.clone(), types)
    }

    #[test]
    fn mismatch_suggests_conversion() {
        let source = format!(
            "{NODE_ID} fn main(): i32 {{ id: NodeId i: i32 i = 1 id = i return 0 }}"
        );
        let (code, help, types) = diagnostic(&source);
        assert_eq!(code, Code::Mismatch);
        assert_eq!(help.as_deref(), Some("convert it: `NodeId(...)`"));
        assert_eq!(types, Some(("NodeId".to_string(), "i32".to_string())));
    }

    #[test]
    fn misspelled_names_get_suggestions() {
        let cases = [
            ("fn main(): i32 { total: i32 total = 1 return totla }", Code::UnknownVariable, "total"),
            ("fn helper(): i32 { return 1 } fn main(): i32 { return hlper() }", Code::UnknownFunction, "helper"),
            ("type Point struct { x: i32 } fn f(p: Piont) {} fn main(): i32 { return 0 }", Code::UnknownType, "Point"),
            (
                "type P struct { value: i32 } fn main(): i32 { p: P p = P(value: 1) return p.valeu }",
                Code::UnknownField,
                "value",
            ),
            (
                "type P struct { x: i32 } fn P.bump() { this.x = 1 }
                 fn main(): i32 { p: P p = P(x: 1) p.bmup() return 0 }",
                Code::UnknownMethod,
                "bump",
            ),
        ];

        for (source, expected_code, suggestion) in cases {
            let (code, help, _) = diagnostic(source);
            assert_eq!(code, expected_code, "{source}");
            assert_eq!(help, Some(format!("did you mean `{suggestion}`?")), "{source}");
        }
    }

    #[test]
    fn flow_errors_have_codes_and_help() {
        let (code, help, _) = diagnostic("fn main(): i32 { x: i32 return x }");
        assert_eq!(code, Code::UseBeforeAssign);
        assert!(help.is_some());

        let (code, help, _) = diagnostic("fn main(): i32 { if true { return 1 } }");
        assert_eq!(code, Code::MissingReturn);
        assert!(help.is_some());
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
