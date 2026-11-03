//! Debug output of the syntax tree in Graphviz dot format.

use std::io::{self, Write};

use super::ast::*;

pub fn write(module: &Module, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "digraph {{")?;

    let mut graph = Graph { out, next_id: 0 };
    let root = graph.node("Module")?;

    for function in &module.functions {
        let child = graph.function(function)?;
        graph.edge(root, child)?;
    }

    writeln!(graph.out, "}}")
}

struct Graph<'a> {
    out: &'a mut dyn Write,
    next_id: usize,
}

type Node = usize;

impl Graph<'_> {
    fn node(&mut self, label: &str) -> io::Result<Node> {
        let id = self.next_id;
        self.next_id += 1;

        writeln!(
            self.out,
            "n{id} [ shape = record, label = \"{}\" ];",
            escape(label)
        )?;

        Ok(id)
    }

    fn edge(&mut self, from: Node, to: Node) -> io::Result<()> {
        writeln!(self.out, "n{from} -> n{to};")
    }

    fn function(&mut self, function: &Function) -> io::Result<Node> {
        let returns = function
            .returns
            .iter()
            .map(Type::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        let node = self.node(&format!(
            "Function|{}|returns: {}",
            function.name.name, returns
        ))?;

        for param in &function.params {
            let child = self.node(&format!("Param|{}: {}", param.name.name, param.ty))?;
            self.edge(node, child)?;
        }

        let body = self.block(&function.body)?;
        self.edge(node, body)?;

        Ok(node)
    }

    fn block(&mut self, block: &Block) -> io::Result<Node> {
        let node = self.node("Block")?;

        for stmt in &block.stmts {
            let child = self.stmt(stmt)?;
            self.edge(node, child)?;
        }

        Ok(node)
    }

    fn stmt(&mut self, stmt: &Stmt) -> io::Result<Node> {
        match &stmt.kind {
            StmtKind::Decl(decl) => self.node(&format!("Decl|{}: {}", decl.name.name, decl.ty)),

            StmtKind::Assign { targets, values } => {
                let node = self.node("Assign")?;

                for target in targets {
                    let child = self.node(&format!("Place|{}", target.name.name))?;
                    self.edge(node, child)?;

                    if let Some(index) = &target.index {
                        let index = self.expr(index)?;
                        self.edge(child, index)?;
                    }
                }

                self.exprs(node, values)?;
                Ok(node)
            }

            StmtKind::Call(call) => self.call(call),

            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                let node = self.node("If")?;

                let child = self.expr(cond)?;
                self.edge(node, child)?;

                let child = self.block(then_block)?;
                self.edge(node, child)?;

                if let Some(else_block) = else_block {
                    let child = self.block(else_block)?;
                    self.edge(node, child)?;
                }

                Ok(node)
            }

            StmtKind::While { cond, body } => {
                let node = self.node("While")?;

                let child = self.expr(cond)?;
                self.edge(node, child)?;

                let child = self.block(body)?;
                self.edge(node, child)?;

                Ok(node)
            }

            StmtKind::Return(values) => {
                let node = self.node("Return")?;
                self.exprs(node, values)?;
                Ok(node)
            }
        }
    }

    fn exprs(&mut self, parent: Node, exprs: &[Expr]) -> io::Result<()> {
        for expr in exprs {
            let child = self.expr(expr)?;
            self.edge(parent, child)?;
        }

        Ok(())
    }

    fn call(&mut self, call: &Call) -> io::Result<Node> {
        let node = self.node(&format!("Call|{}", call.callee.name))?;
        self.exprs(node, &call.args)?;
        Ok(node)
    }

    fn expr(&mut self, expr: &Expr) -> io::Result<Node> {
        match &expr.kind {
            ExprKind::Int(value) => self.node(&format!("Int|{value}")),
            ExprKind::Bool(value) => self.node(&format!("Bool|{value}")),
            ExprKind::Var(ident) => self.node(&format!("Var|{}", ident.name)),
            ExprKind::Call(call) => self.call(call),
            ExprKind::ArrayList(elems) => {
                let node = self.node("Array")?;
                self.exprs(node, elems)?;
                Ok(node)
            }
            ExprKind::ArrayRepeat(value, len) => {
                let node = self.node(&format!("Array|len: {len}"))?;
                let child = self.expr(value)?;
                self.edge(node, child)?;
                Ok(node)
            }
            ExprKind::Index { base, index } => {
                let node = self.node("Index")?;

                let child = self.expr(base)?;
                self.edge(node, child)?;

                let child = self.expr(index)?;
                self.edge(node, child)?;

                Ok(node)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let node = self.node(op.as_str())?;

                let child = self.expr(lhs)?;
                self.edge(node, child)?;

                let child = self.expr(rhs)?;
                self.edge(node, child)?;

                Ok(node)
            }
        }
    }
}

/// Escapes characters that have special meaning in record labels.
fn escape(label: &str) -> String {
    let mut out = String::with_capacity(label.len());

    for c in label.chars() {
        if matches!(c, '<' | '>' | '{' | '}' | '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }

    out
}
