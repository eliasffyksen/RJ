//! Lowers the checked program ([`hir`]) into the IR.

use super::*;
use crate::hir;

pub fn lower(program: &hir::Program) -> Program {
    let writes_this = writes_this(program);

    Program {
        functions: program
            .functions
            .iter()
            .enumerate()
            .map(|(i, function)| lower_function(program, &writes_this, writes_this[i], function))
            .collect(),
        examples: program
            .examples
            .iter()
            .map(|example| Example {
                function: lower_function(program, &writes_this, false, &example.function),
                compare: example.compare,
                span: example.span,
            })
            .collect(),
        structs: program.structs.clone(),
        main: program.main,
    }
}

fn lower_function(
    program: &hir::Program,
    writes_this: &[bool],
    writes_own_this: bool,
    function: &hir::Function,
) -> Function {
    // HIR locals keep their ids; return slots and temporaries come after.
    let locals = function
        .locals
        .iter()
        .enumerate()
        .map(|(i, local)| Local {
            ty: local.ty.clone(),
            kind: if function.params.contains(&LocalId(i)) {
                LocalKind::Param(local.name.clone())
            } else {
                LocalKind::Var(local.name.clone())
            },
        })
        .collect();

    let mut builder = Builder {
        program,
        writes_this,
        locals,
        returns: vec![],
        this_slot: None,
        blocks: vec![],
        current: None,
        span: function.span,
    };

    builder.returns = function
        .returns
        .iter()
        .map(|ty| builder.new_local(ty.clone(), LocalKind::Return))
        .collect();

    // A method that changes `this` hands it back as an extra, last return
    // value, which the caller writes back to the receiver. Every return
    // writes it from `this`, so `this` comes out as an in/out parameter.
    if writes_own_this {
        let this = function.params[0];
        let ty = builder.locals[this.0].ty.clone();
        let slot = builder.new_local(ty, LocalKind::Return);

        builder.returns.push(slot);
        builder.this_slot = Some((this, slot));
    }

    let entry = builder.new_block();
    builder.current = Some(entry);
    builder.block(&function.body);

    // The checker guarantees that only functions without return values can
    // fall off the end of their body.
    if builder.current.is_some() {
        debug_assert!(function.returns.is_empty());
        builder.return_this();
        builder.terminate(Terminator::Return);
    }

    Function {
        name: function.name.clone(),
        params: function.params.clone(),
        returns: builder.returns,
        locals: builder.locals,
        blocks: builder
            .blocks
            .into_iter()
            .map(|(stmts, spans, terminator)| BasicBlock {
                stmts,
                spans,
                terminator: terminator.expect("unterminated block"),
            })
            .collect(),
    }
}

struct Builder<'a> {
    program: &'a hir::Program,
    /// Which methods change `this`, by `FuncId`.
    writes_this: &'a [bool],
    locals: Vec<Local>,
    returns: Vec<LocalId>,
    /// `this` and the return slot it is handed back in, for methods that
    /// change it.
    this_slot: Option<(LocalId, LocalId)>,
    blocks: Vec<(Vec<Stmt>, Vec<Span>, Option<Terminator>)>,
    /// The block statements are appended to; `None` after a `return`.
    current: Option<BlockId>,
    /// The source statement being lowered.
    span: Span,
}

impl Builder<'_> {
    fn new_local(&mut self, ty: Type, kind: LocalKind) -> LocalId {
        self.locals.push(Local { ty, kind });
        LocalId(self.locals.len() - 1)
    }

    fn new_temp(&mut self, ty: Type) -> LocalId {
        self.new_local(ty, LocalKind::Temp)
    }

    fn new_block(&mut self) -> BlockId {
        self.blocks.push((vec![], vec![], None));
        BlockId(self.blocks.len() - 1)
    }

    fn push(&mut self, stmt: Stmt) {
        let current = self.current.expect("statement in unreachable code");
        self.blocks[current.0].0.push(stmt);
        self.blocks[current.0].1.push(self.span);
    }

    fn terminate(&mut self, terminator: Terminator) {
        let current = self.current.take().expect("terminator in unreachable code");
        self.blocks[current.0].2 = Some(terminator);
    }

    fn returns_of(&self, call: &hir::Call) -> Vec<Type> {
        self.program.function(call.func).returns.clone()
    }

    fn block(&mut self, block: &hir::Block) {
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &hir::Stmt) {
        let outer = std::mem::replace(&mut self.span, stmt.span);
        self.stmt_kind(stmt);
        self.span = outer;
    }

    fn stmt_kind(&mut self, stmt: &hir::Stmt) {
        match &stmt.kind {
            hir::StmtKind::Assign { targets, values } => self.assign(targets, values),

            hir::StmtKind::Call(call) => {
                let dests = self
                    .returns_of(call)
                    .into_iter()
                    .map(|ty| self.new_temp(ty))
                    .collect();
                self.call(call, dests);
            }

            hir::StmtKind::If {
                cond,
                then_block,
                else_block,
            } => self.if_(cond, then_block, else_block),

            hir::StmtKind::While { cond, body } => self.while_(cond, body),

            hir::StmtKind::Return(values) => {
                // Return slots are never read by the function itself, so the
                // values can be written straight into them in order.
                let mut slots = self.returns.clone().into_iter();

                for value in values {
                    match value {
                        hir::Value::Single(expr) => {
                            let slot = slots.next().unwrap();
                            self.expr_into(expr, slot);
                        }
                        hir::Value::Multi(call) => {
                            let count = self.program.function(call.func).returns.len();
                            let dests = slots.by_ref().take(count).collect();
                            self.call(call, dests);
                        }
                    }
                }

                self.return_this();
                self.terminate(Terminator::Return);
            }
        }
    }

    fn return_this(&mut self) {
        if let Some((this, slot)) = self.this_slot {
            self.push(Stmt::Assign {
                dest: slot,
                value: Rvalue::Use(Operand::Local(this)),
            });
        }
    }

    fn assign(&mut self, targets: &[hir::Place], values: &[hir::Value]) {
        let whole_locals = targets
            .iter()
            .map(|target| target.path.is_empty().then_some(target.local))
            .collect::<Option<Vec<_>>>();

        match (targets, values, whole_locals) {
            ([target], [hir::Value::Single(expr)], _) if target.path.is_empty() => {
                self.expr_into(expr, target.local)
            }

            // `g.nodes[i] = f(g.nodes[i])`: the call writes straight into the
            // place, and an argument naming the same place shares its
            // lowered path, so codegen can see it's the same place.
            ([target], [hir::Value::Single(hir::Expr { kind: hir::ExprKind::Call(call), .. })], _) => {
                let path = self.path(&target.path, false);
                let dest = Place { local: target.local, path };
                self.call_to(call, vec![dest.clone()], Some((target, &dest)));
            }

            ([target], [hir::Value::Single(expr)], _) => {
                let value = self.operand(expr);
                let path = self.path(&target.path, false);

                self.push(Stmt::SetPlace {
                    local: target.local,
                    path,
                    value,
                });
            }

            (_, [hir::Value::Multi(call)], Some(dests)) => self.call(call, dests),

            // No value reads a target, so write each one straight into place.
            (_, _, Some(dests)) if !values.iter().any(|value| reads_any(value, &dests)) => {
                let mut dests = dests.into_iter();

                for value in values {
                    match value {
                        hir::Value::Single(expr) => self.expr_into(expr, dests.next().unwrap()),
                        hir::Value::Multi(call) => {
                            let count = self.program.function(call.func).returns.len();
                            let call_dests = dests.by_ref().take(count).collect();
                            self.call(call, call_dests);
                        }
                    }
                }
            }

            // `a, b = b, a` or `a[i], a[j] = a[j], a[i]`: every value and
            // index must be evaluated before any target is written, so go
            // through temporaries.
            _ => {
                let temps = self.values_into_temps(values);

                let paths = targets
                    .iter()
                    .map(|target| self.path(&target.path, true))
                    .collect::<Vec<_>>();

                for ((target, temp), path) in targets.iter().zip(temps).zip(paths) {
                    let value = Operand::Local(temp);

                    if path.is_empty() {
                        self.push(Stmt::Assign {
                            dest: target.local,
                            value: Rvalue::Use(value),
                        });
                    } else {
                        self.push(Stmt::SetPlace {
                            local: target.local,
                            path,
                            value,
                        });
                    }
                }
            }
        }
    }

    /// Lowers a place path, evaluating its indexes. With `stable`, indexes
    /// keep their value even if locals are written before the path is used.
    fn path(&mut self, path: &[hir::PlaceElem], stable: bool) -> Vec<Elem> {
        path.iter()
            .map(|elem| match elem {
                hir::PlaceElem::Field(field) => Elem::Field(*field),
                hir::PlaceElem::Index(index) if stable => Elem::Index(self.stable_operand(index)),
                hir::PlaceElem::Index(index) => Elem::Index(self.operand(index)),
            })
            .collect()
    }

    /// Like [`Self::operand`], but copies locals into a temporary so the
    /// operand keeps its value if the local is written afterwards.
    fn stable_operand(&mut self, expr: &hir::Expr) -> Operand {
        match self.operand(expr) {
            Operand::Local(id) => {
                let temp = self.new_temp(expr.ty.clone());
                self.push(Stmt::Assign {
                    dest: temp,
                    value: Rvalue::Use(Operand::Local(id)),
                });
                Operand::Local(temp)
            }
            operand => operand,
        }
    }

    fn if_(&mut self, cond: &hir::Expr, then_block: &hir::Block, else_block: &hir::Block) {
        let cond = self.operand(cond);
        let then_start = self.new_block();

        // Without an `else`, branch straight to the code after the `if`.
        if else_block.stmts.is_empty() {
            let join = self.new_block();

            self.terminate(Terminator::Branch {
                cond,
                then_block: then_start,
                else_block: join,
            });

            self.current = Some(then_start);
            self.block(then_block);

            if self.current.is_some() {
                self.terminate(Terminator::Goto(join));
            }

            self.current = Some(join);
            return;
        }

        let else_start = self.new_block();

        self.terminate(Terminator::Branch {
            cond,
            then_block: then_start,
            else_block: else_start,
        });

        self.current = Some(then_start);
        self.block(then_block);
        let then_end = self.current.take();

        self.current = Some(else_start);
        self.block(else_block);
        let else_end = self.current.take();

        // Only create a join block if some branch falls through.
        if then_end.is_none() && else_end.is_none() {
            return;
        }

        let join = self.new_block();

        for end in [then_end, else_end].into_iter().flatten() {
            self.current = Some(end);
            self.terminate(Terminator::Goto(join));
        }

        self.current = Some(join);
    }

    /// ```text
    /// current: goto header
    /// header:  branch cond, body, exit
    /// body:    ...; goto header
    /// exit:
    /// ```
    fn while_(&mut self, cond: &hir::Expr, body: &hir::Block) {
        let header = self.new_block();
        self.terminate(Terminator::Goto(header));

        // The condition is evaluated in the header, on every iteration.
        self.current = Some(header);
        let cond = self.operand(cond);
        let body_start = self.new_block();
        let exit = self.new_block();

        self.terminate(Terminator::Branch {
            cond,
            then_block: body_start,
            else_block: exit,
        });

        self.current = Some(body_start);
        self.block(body);

        if self.current.is_some() {
            self.terminate(Terminator::Goto(header));
        }

        self.current = Some(exit);
    }

    fn call(&mut self, call: &hir::Call, dests: Vec<LocalId>) {
        let dests = dests.into_iter().map(Place::whole).collect();
        self.call_to(call, dests, None);
    }

    /// `shared` is a destination place with its source form; an argument
    /// naming the same place is passed as that exact place.
    fn call_to(
        &mut self,
        call: &hir::Call,
        mut dests: Vec<Place>,
        shared: Option<(&hir::Place, &Place)>,
    ) {
        let mut args = vec![];

        if let Some(receiver) = &call.receiver {
            let method = self.program.function(call.func);
            let ty = method.locals[method.params[0].0].ty.clone();
            let writes = self.writes_this[call.func.0];

            match receiver {
                hir::Receiver::Place(place) => {
                    let place = Place {
                        local: place.local,
                        path: self.path(&place.path, false),
                    };

                    args.push(if place.path.is_empty() {
                        CallArg::Value(Operand::Local(place.local))
                    } else if ty.is_aggregate() {
                        CallArg::Place(place.clone())
                    } else {
                        // Scalars are passed by value.
                        let temp = self.new_temp(ty.clone());
                        self.push(Stmt::Assign {
                            dest: temp,
                            value: Rvalue::Read(place.local, place.path.clone()),
                        });
                        CallArg::Value(Operand::Local(temp))
                    });

                    // The changed `this` is written back to the receiver.
                    if writes {
                        dests.push(place);
                    }
                }

                // Changes to a value that isn't a variable are discarded.
                hir::Receiver::Value(expr) => {
                    let operand = self.operand(expr);
                    args.push(CallArg::Value(operand));

                    if writes {
                        let dest = match operand {
                            Operand::Local(temp) => temp,
                            _ => self.new_temp(ty),
                        };
                        dests.push(Place::whole(dest));
                    }
                }
            }
        }

        args.extend(self.args(&call.args, shared));

        self.push(Stmt::Call {
            func: call.func,
            args,
            dests,
        });
    }

    fn args(&mut self, values: &[hir::Value], shared: Option<(&hir::Place, &Place)>) -> Vec<CallArg> {
        let mut operands = vec![];

        for value in values {
            match value {
                hir::Value::Single(expr) if expr.ty.is_aggregate() => {
                    if let Some((source, place)) = shared {
                        if expr_is_place(expr, source) {
                            operands.push(CallArg::Place(place.clone()));
                            continue;
                        }
                    }

                    match self.place_read(expr) {
                        Some(place) => operands.push(CallArg::Place(place)),
                        None => operands.push(CallArg::Value(self.operand(expr))),
                    }
                }
                hir::Value::Single(expr) => operands.push(CallArg::Value(self.operand(expr))),
                hir::Value::Multi(call) => {
                    let temps = self
                        .returns_of(call)
                        .into_iter()
                        .map(|ty| self.new_temp(ty))
                        .collect::<Vec<_>>();
                    self.call(call, temps.clone());
                    operands.extend(temps.into_iter().map(|t| CallArg::Value(Operand::Local(t))));
                }
            }
        }

        operands
    }

    /// `local.path` for an expression like `g.nodes[i]` that reads a part of
    /// a local, with its indexes evaluated.
    fn place_read(&mut self, expr: &hir::Expr) -> Option<Place> {
        let (base, steps) = chain(expr);

        let hir::ExprKind::Local(local) = base.kind else {
            return None;
        };
        if steps.is_empty() {
            return None;
        }

        let path = self.steps(&steps);
        Some(Place { local, path })
    }

    /// Lowers the steps of an index/field chain, innermost first.
    fn steps(&mut self, steps: &[&hir::Expr]) -> Vec<Elem> {
        steps
            .iter()
            .map(|step| match &step.kind {
                hir::ExprKind::Index { index, .. } => Elem::Index(self.operand(index)),
                hir::ExprKind::Field { field, .. } => Elem::Field(*field),
                _ => unreachable!(),
            })
            .collect()
    }

    fn values_into_temps(&mut self, values: &[hir::Value]) -> Vec<LocalId> {
        let mut temps = vec![];

        for value in values {
            match value {
                hir::Value::Single(expr) => {
                    let temp = self.new_temp(expr.ty.clone());
                    self.expr_into(expr, temp);
                    temps.push(temp);
                }
                hir::Value::Multi(call) => {
                    let dests = self
                        .returns_of(call)
                        .into_iter()
                        .map(|ty| self.new_temp(ty))
                        .collect::<Vec<_>>();
                    self.call(call, dests.clone());
                    temps.extend(dests);
                }
            }
        }

        temps
    }

    /// Lowers an expression to an operand, using a temporary if needed.
    fn operand(&mut self, expr: &hir::Expr) -> Operand {
        match &expr.kind {
            // Converted values are stored the same way.
            hir::ExprKind::Convert(inner) => self.operand(inner),
            hir::ExprKind::Int(value) => Operand::Int(*value),
            hir::ExprKind::Bool(value) => Operand::Bool(*value),
            hir::ExprKind::Local(id) => Operand::Local(*id),
            hir::ExprKind::Call(_)
            | hir::ExprKind::Binary { .. }
            | hir::ExprKind::ArrayList(_)
            | hir::ExprKind::ArrayRepeat(..)
            | hir::ExprKind::Index { .. }
            | hir::ExprKind::Field { .. }
            | hir::ExprKind::StructLit(..) => {
                let temp = self.new_temp(expr.ty.clone());
                self.expr_into(expr, temp);
                Operand::Local(temp)
            }
        }
    }

    /// Lowers an expression, storing its value in `dest`.
    fn expr_into(&mut self, expr: &hir::Expr, dest: LocalId) {
        match &expr.kind {
            hir::ExprKind::Convert(inner) => self.expr_into(inner, dest),

            hir::ExprKind::Call(call) => self.call(call, vec![dest]),

            hir::ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.operand(lhs);
                let rhs = self.operand(rhs);

                self.push(Stmt::Assign {
                    dest,
                    value: Rvalue::Binary(*op, lhs, rhs),
                });
            }

            hir::ExprKind::ArrayList(elems) => {
                let elems = elems.iter().map(|elem| self.operand(elem)).collect();

                self.push(Stmt::Assign {
                    dest,
                    value: Rvalue::Array(elems),
                });
            }

            hir::ExprKind::ArrayRepeat(value, len) => {
                let value = self.operand(value);

                self.push(Stmt::Assign {
                    dest,
                    value: Rvalue::Repeat(value, *len),
                });
            }

            hir::ExprKind::StructLit(_, fields) => {
                let fields = fields.iter().map(|field| self.operand(field)).collect();

                self.push(Stmt::Assign {
                    dest,
                    value: Rvalue::Struct(fields),
                });
            }

            // `a.items[i].x`: one read along a path from the innermost base.
            hir::ExprKind::Index { .. } | hir::ExprKind::Field { .. } => {
                let (base, steps) = chain(expr);

                // Arrays and structs have no constant operands, so the base
                // is always a local.
                let Operand::Local(local) = self.operand(base) else {
                    unreachable!("aggregate operand is not a local");
                };
                let path = self.steps(&steps);

                self.push(Stmt::Assign {
                    dest,
                    value: Rvalue::Read(local, path),
                });
            }

            _ => {
                let operand = self.operand(expr);

                self.push(Stmt::Assign {
                    dest,
                    value: Rvalue::Use(operand),
                });
            }
        }
    }
}

fn reads_any(value: &hir::Value, locals: &[LocalId]) -> bool {
    match value {
        hir::Value::Single(expr) => expr_reads_any(expr, locals),
        hir::Value::Multi(call) => call_reads_any(call, locals),
    }
}

fn call_reads_any(call: &hir::Call, locals: &[LocalId]) -> bool {
    let receiver = match &call.receiver {
        Some(hir::Receiver::Place(place)) => {
            locals.contains(&place.local)
                || place.path.iter().any(|elem| match elem {
                    hir::PlaceElem::Index(index) => expr_reads_any(index, locals),
                    hir::PlaceElem::Field(_) => false,
                })
        }
        Some(hir::Receiver::Value(expr)) => expr_reads_any(expr, locals),
        None => false,
    };

    receiver || call.args.iter().any(|arg| reads_any(arg, locals))
}

fn expr_reads_any(expr: &hir::Expr, locals: &[LocalId]) -> bool {
    match &expr.kind {
        hir::ExprKind::Int(_) | hir::ExprKind::Bool(_) => false,
        hir::ExprKind::Local(id) => locals.contains(id),
        hir::ExprKind::Call(call) => call_reads_any(call, locals),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            expr_reads_any(lhs, locals) || expr_reads_any(rhs, locals)
        }
        hir::ExprKind::ArrayList(elems) => elems.iter().any(|e| expr_reads_any(e, locals)),
        hir::ExprKind::ArrayRepeat(value, _) => expr_reads_any(value, locals),
        hir::ExprKind::Index { base, index } => {
            expr_reads_any(base, locals) || expr_reads_any(index, locals)
        }
        hir::ExprKind::Field { base, .. } => expr_reads_any(base, locals),
        hir::ExprKind::Convert(inner) => expr_reads_any(inner, locals),
        hir::ExprKind::StructLit(_, fields) => fields.iter().any(|f| expr_reads_any(f, locals)),
    }
}

/// Splits `a.b[i].c` into its base `a` and its steps, innermost first.
fn chain(expr: &hir::Expr) -> (&hir::Expr, Vec<&hir::Expr>) {
    let mut steps = vec![];
    let mut base = expr;

    while let hir::ExprKind::Index { base: inner, .. } | hir::ExprKind::Field { base: inner, .. } =
        &base.kind
    {
        steps.push(base);
        base = inner;
    }

    steps.reverse();
    (base, steps)
}

/// Whether `expr` reads exactly the place `place`, with equal indexes.
/// Expressions are pure, so equal index expressions have equal values.
fn expr_is_place(expr: &hir::Expr, place: &hir::Place) -> bool {
    let (base, steps) = chain(expr);

    matches!(base.kind, hir::ExprKind::Local(id) if id == place.local)
        && steps.len() == place.path.len()
        && steps.iter().zip(&place.path).all(|(step, elem)| match (&step.kind, elem) {
            (hir::ExprKind::Index { index, .. }, hir::PlaceElem::Index(other)) => {
                expr_eq(index, other)
            }
            (hir::ExprKind::Field { field, .. }, hir::PlaceElem::Field(other)) => field == other,
            _ => false,
        })
}

/// Structural equality of simple expressions, ignoring spans.
fn expr_eq(a: &hir::Expr, b: &hir::Expr) -> bool {
    use hir::ExprKind::*;

    match (&a.kind, &b.kind) {
        (Int(x), Int(y)) => x == y,
        (Bool(x), Bool(y)) => x == y,
        (Local(x), Local(y)) => x == y,
        (
            Binary { op, lhs, rhs },
            Binary {
                op: op2,
                lhs: lhs2,
                rhs: rhs2,
            },
        ) => op == op2 && expr_eq(lhs, lhs2) && expr_eq(rhs, rhs2),
        (Index { base, index }, Index { base: base2, index: index2 }) => {
            expr_eq(base, base2) && expr_eq(index, index2)
        }
        (Field { base, field }, Field { base: base2, field: field2 }) => {
            field == field2 && expr_eq(base, base2)
        }
        _ => false,
    }
}

/// Which methods change `this`: by assigning to it or a part of it, or by
/// calling a method that changes it on `this` or a part of it. Iterates to
/// a fixpoint, since methods can call each other.
fn writes_this(program: &hir::Program) -> Vec<bool> {
    let mut writes = vec![false; program.functions.len()];

    loop {
        let mut changed = false;

        for (i, function) in program.functions.iter().enumerate() {
            if function.is_method && !writes[i] {
                let this = function.params[0];

                if block_writes(&function.body, this, &writes) {
                    writes[i] = true;
                    changed = true;
                }
            }
        }

        if !changed {
            return writes;
        }
    }
}

fn block_writes(block: &hir::Block, this: LocalId, writes: &[bool]) -> bool {
    block.stmts.iter().any(|stmt| match &stmt.kind {
        hir::StmtKind::Assign { targets, values } => {
            targets.iter().any(|target| target.local == this)
                || values.iter().any(|value| value_writes(value, this, writes))
        }
        hir::StmtKind::Call(call) => call_writes(call, this, writes),
        hir::StmtKind::If {
            cond,
            then_block,
            else_block,
        } => {
            expr_writes(cond, this, writes)
                || block_writes(then_block, this, writes)
                || block_writes(else_block, this, writes)
        }
        hir::StmtKind::While { cond, body } => {
            expr_writes(cond, this, writes) || block_writes(body, this, writes)
        }
        hir::StmtKind::Return(values) => values.iter().any(|v| value_writes(v, this, writes)),
    })
}

fn value_writes(value: &hir::Value, this: LocalId, writes: &[bool]) -> bool {
    match value {
        hir::Value::Single(expr) => expr_writes(expr, this, writes),
        hir::Value::Multi(call) => call_writes(call, this, writes),
    }
}

fn call_writes(call: &hir::Call, this: LocalId, writes: &[bool]) -> bool {
    let receiver = match &call.receiver {
        Some(hir::Receiver::Place(place)) => place.local == this && writes[call.func.0],
        Some(hir::Receiver::Value(expr)) => expr_writes(expr, this, writes),
        None => false,
    };

    receiver || call.args.iter().any(|arg| value_writes(arg, this, writes))
}

fn expr_writes(expr: &hir::Expr, this: LocalId, writes: &[bool]) -> bool {
    match &expr.kind {
        hir::ExprKind::Int(_) | hir::ExprKind::Bool(_) | hir::ExprKind::Local(_) => false,
        hir::ExprKind::Call(call) => call_writes(call, this, writes),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            expr_writes(lhs, this, writes) || expr_writes(rhs, this, writes)
        }
        hir::ExprKind::ArrayList(elems) | hir::ExprKind::StructLit(_, elems) => {
            elems.iter().any(|e| expr_writes(e, this, writes))
        }
        hir::ExprKind::ArrayRepeat(value, _) => expr_writes(value, this, writes),
        hir::ExprKind::Index { base, index } => {
            expr_writes(base, this, writes) || expr_writes(index, this, writes)
        }
        hir::ExprKind::Field { base, .. } | hir::ExprKind::Convert(base) => {
            expr_writes(base, this, writes)
        }
    }
}
