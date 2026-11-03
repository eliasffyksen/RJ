//! Lowers the checked program ([`hir`]) into the IR.

use super::*;
use crate::hir;

pub fn lower(program: &hir::Program) -> Program {
    Program {
        functions: program
            .functions
            .iter()
            .map(|function| lower_function(program, function))
            .collect(),
        main: program.main,
    }
}

fn lower_function(program: &hir::Program, function: &hir::Function) -> Function {
    // HIR locals keep their ids; return slots and temporaries come after.
    let locals = function
        .locals
        .iter()
        .enumerate()
        .map(|(i, local)| Local {
            ty: local.ty,
            kind: if function.params.contains(&LocalId(i)) {
                LocalKind::Param(local.name.clone())
            } else {
                LocalKind::Var(local.name.clone())
            },
        })
        .collect();

    let mut builder = Builder {
        program,
        locals,
        returns: vec![],
        blocks: vec![],
        current: None,
    };

    builder.returns = function
        .returns
        .iter()
        .map(|ty| builder.new_local(*ty, LocalKind::Return))
        .collect();

    let entry = builder.new_block();
    builder.current = Some(entry);
    builder.block(&function.body);

    // The checker guarantees that only functions without return values can
    // fall off the end of their body.
    if builder.current.is_some() {
        debug_assert!(function.returns.is_empty());
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
            .map(|(stmts, terminator)| BasicBlock {
                stmts,
                terminator: terminator.expect("unterminated block"),
            })
            .collect(),
    }
}

struct Builder<'a> {
    program: &'a hir::Program,
    locals: Vec<Local>,
    returns: Vec<LocalId>,
    blocks: Vec<(Vec<Stmt>, Option<Terminator>)>,
    /// The block statements are appended to; `None` after a `return`.
    current: Option<BlockId>,
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
        self.blocks.push((vec![], None));
        BlockId(self.blocks.len() - 1)
    }

    fn push(&mut self, stmt: Stmt) {
        let current = self.current.expect("statement in unreachable code");
        self.blocks[current.0].0.push(stmt);
    }

    fn terminate(&mut self, terminator: Terminator) {
        let current = self.current.take().expect("terminator in unreachable code");
        self.blocks[current.0].1 = Some(terminator);
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

                self.terminate(Terminator::Return);
            }
        }
    }

    fn assign(&mut self, targets: &[LocalId], values: &[hir::Value]) {
        match (targets, values) {
            ([target], [hir::Value::Single(expr)]) => self.expr_into(expr, *target),

            (_, [hir::Value::Multi(call)]) => self.call(call, targets.to_vec()),

            // `a, b = b, a`: every value must be read before any target is
            // written, so go through temporaries.
            _ => {
                let temps = self.values_into_temps(values);

                for (target, temp) in targets.iter().zip(temps) {
                    self.push(Stmt::Assign {
                        dest: *target,
                        value: Rvalue::Use(Operand::Local(temp)),
                    });
                }
            }
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
        let args = self.args(&call.args);

        self.push(Stmt::Call {
            func: call.func,
            args,
            dests,
        });
    }

    fn args(&mut self, values: &[hir::Value]) -> Vec<Operand> {
        let mut operands = vec![];

        for value in values {
            match value {
                hir::Value::Single(expr) => operands.push(self.operand(expr)),
                hir::Value::Multi(call) => {
                    let temps = self
                        .returns_of(call)
                        .into_iter()
                        .map(|ty| self.new_temp(ty))
                        .collect::<Vec<_>>();
                    self.call(call, temps.clone());
                    operands.extend(temps.into_iter().map(Operand::Local));
                }
            }
        }

        operands
    }

    fn values_into_temps(&mut self, values: &[hir::Value]) -> Vec<LocalId> {
        let mut temps = vec![];

        for value in values {
            match value {
                hir::Value::Single(expr) => {
                    let temp = self.new_temp(expr.ty);
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
            hir::ExprKind::Int(value) => Operand::Int(*value),
            hir::ExprKind::Bool(value) => Operand::Bool(*value),
            hir::ExprKind::Local(id) => Operand::Local(*id),
            hir::ExprKind::Call(_) | hir::ExprKind::Binary { .. } => {
                let temp = self.new_temp(expr.ty);
                self.expr_into(expr, temp);
                Operand::Local(temp)
            }
        }
    }

    /// Lowers an expression, storing its value in `dest`.
    fn expr_into(&mut self, expr: &hir::Expr, dest: LocalId) {
        match &expr.kind {
            hir::ExprKind::Call(call) => self.call(call, vec![dest]),

            hir::ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.operand(lhs);
                let rhs = self.operand(rhs);

                self.push(Stmt::Assign {
                    dest,
                    value: Rvalue::Binary(*op, lhs, rhs),
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
