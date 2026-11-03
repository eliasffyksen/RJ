//! The canonical formatter, `rjc --fmt`.
//!
//! Prints the syntax tree back as source in one fixed style: two-space
//! indentation, one item or statement per line, a single space around
//! binary operators. Top-level items are separated by one blank line;
//! inside blocks, a blank line is kept wherever the source had at least
//! one.
//!
//! The parser drops comments, so they are collected from the source by
//! position and re-attached: a comment on its own line stays above the next
//! item or statement, and a comment after code on the same line stays at the
//! end of that line.

use super::ast::*;

pub fn format(source: &str, module: &Module) -> String {
    let mut f = Formatter {
        source,
        comments: comments(source),
        next_comment: 0,
        out: String::new(),
        indent: 0,
    };

    // The tree keeps types, functions and examples apart; print them in
    // source order.
    let mut items = vec![];
    items.extend(module.types.iter().map(|t| (t.span.start, Item::Type(t))));
    items.extend(module.functions.iter().map(|f| (f.span.start, Item::Function(f))));
    items.extend(module.examples.iter().map(|e| (e.span.start, Item::Example(e))));
    items.sort_by_key(|(start, _)| *start);

    let mut previous: Option<&Item> = None;

    for (start, item) in &items {
        // Consecutive examples stay together; everything else gets a
        // blank line between items.
        let together = matches!((previous, item), (Some(Item::Example(_)), Item::Example(_)))
            && f.blank_lines_before(*start) == 0;

        if previous.is_some() && !together {
            f.out.push('\n');
        }

        f.comments_above(*start);

        match item {
            Item::Type(def) => f.type_def(def),
            Item::Function(function) => f.function(function),
            Item::Example(example) => {
                let line = format!("example {}", f.expr(&example.expr));
                f.line(&line, example.span.end);
            }
        }

        previous = Some(item);
    }

    // Comments after the last item.
    f.leading_comments(source.len());

    f.out
}

enum Item<'a> {
    Type(&'a TypeDef),
    Function(&'a Function),
    Example(&'a Example),
}

struct Comment {
    start: usize,
    end: usize,
}

/// Every `// ...` comment in the source. RJ has no string literals, so
/// `//` always starts a comment.
fn comments(source: &str) -> Vec<Comment> {
    let mut comments = vec![];
    let mut rest = 0;

    while let Some(offset) = source[rest..].find("//") {
        let start = rest + offset;
        let end = source[start..].find('\n').map_or(source.len(), |i| start + i);
        comments.push(Comment { start, end });
        rest = end;
    }

    comments
}

struct Formatter<'a> {
    source: &'a str,
    comments: Vec<Comment>,
    next_comment: usize,
    out: String,
    indent: usize,
}

impl Formatter<'_> {
    fn comment_text(&self, comment: &Comment) -> &str {
        self.source[comment.start..comment.end].trim_end()
    }

    /// Writes one line at the current indentation. `end` is where the
    /// line's code ends in the source; a comment after it on the same
    /// source line is kept at the end of this line.
    fn line(&mut self, text: &str, end: usize) {
        let end = self.code_end(end);
        self.out.push_str(&"  ".repeat(self.indent));
        self.out.push_str(text);

        if let Some(comment) = self.comments.get(self.next_comment) {
            let between = &self.source[end.min(comment.start)..comment.start];
            if comment.start >= end && !between.contains('\n') {
                let text = self.comment_text(comment).to_string();
                self.out.push(' ');
                self.out.push_str(&text);
                self.next_comment += 1;
            }
        }

        self.out.push('\n');
    }

    /// Writes the comments before `pos`, keeping a blank line between them
    /// and the code at `pos` if the source has one.
    fn comments_above(&mut self, pos: usize) {
        let before = self.next_comment;
        self.leading_comments(pos);

        if self.next_comment > before && self.blank_lines_before(pos) > 0 {
            self.out.push('\n');
        }
    }

    /// Moves `pos` back over whitespace and comments, to where code ends.
    /// Parser spans can extend over what follows the code.
    fn code_end(&self, mut pos: usize) -> usize {
        loop {
            pos = self.source[..pos].trim_end().len();

            match self.comments.iter().find(|c| c.start < pos && c.end >= pos) {
                Some(comment) => pos = comment.start,
                None => return pos,
            }
        }
    }

    /// Writes the comments that start before `pos` on their own lines.
    fn leading_comments(&mut self, pos: usize) {
        while let Some(comment) = self.comments.get(self.next_comment) {
            if comment.start >= pos {
                break;
            }

            let text = self.comment_text(comment).to_string();
            let start = comment.start;

            // Keep a blank line between a comment and what came before.
            if self.next_comment > 0 && !self.out.is_empty() && self.blank_lines_before(start) > 0 {
                if !self.out.ends_with("\n\n") && !self.out.ends_with("{\n") {
                    self.out.push('\n');
                }
            }

            self.out.push_str(&"  ".repeat(self.indent));
            self.out.push_str(&text);
            self.out.push('\n');
            self.next_comment += 1;
        }
    }

    /// The number of empty lines right before `pos` in the source.
    fn blank_lines_before(&self, pos: usize) -> usize {
        let before = &self.source[..pos];
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let mut rest = &before[..line_start];
        let mut blanks: usize = 0;

        // Walk back over lines that hold only whitespace.
        while let Some(stripped) = rest.strip_suffix('\n') {
            let start = stripped.rfind('\n').map_or(0, |i| i + 1);
            if !stripped[start..].trim().is_empty() {
                break;
            }
            blanks += 1;
            rest = &stripped[..start];
        }

        blanks
    }

    fn type_def(&mut self, def: &TypeDef) {
        match &def.body {
            TypeDefBody::Type(ty) => {
                let line = format!("type {} {}", def.name.name, type_expr(ty));
                self.line(&line, def.span.end);
            }
            TypeDefBody::Struct(fields) if fields.is_empty() => {
                let line = format!("type {} struct {{}}", def.name.name);
                self.line(&line, def.span.end);
            }
            TypeDefBody::Struct(fields) => {
                let header = format!("type {} struct {{", def.name.name);
                self.line(&header, fields[0].span.start.min(def.name.span.end));
                self.indent += 1;

                for field in fields {
                    self.comments_above(field.span.start);
                    let line = format!("{}: {},", field.name.name, type_expr(&field.ty));
                    // A comment after the comma belongs to this field.
                    let end = self.after_comma(field.span.end);
                    self.line(&line, end);
                }

                self.leading_comments(def.span.end);
                self.indent -= 1;
                self.line("}", def.span.end);
            }
        }
    }

    /// The position after a `,` that follows `pos`, if there is one.
    fn after_comma(&self, pos: usize) -> usize {
        let rest = &self.source[pos..];
        let trimmed = rest.trim_start_matches([' ', '\t']);
        if trimmed.starts_with(',') {
            pos + (rest.len() - trimmed.len()) + 1
        } else {
            pos
        }
    }

    fn function(&mut self, function: &Function) {
        let name = match &function.receiver {
            Some(receiver) => format!("{}.{}", receiver.name, function.name.name),
            None => function.name.name.clone(),
        };
        let params = function
            .params
            .iter()
            .map(|param| format!("{}: {}", param.name.name, type_expr(&param.ty)))
            .collect::<Vec<_>>()
            .join(", ");
        let returns = if function.returns.is_empty() {
            String::new()
        } else {
            let types = function.returns.iter().map(type_expr).collect::<Vec<_>>();
            format!(": {}", types.join(", "))
        };

        let header = format!("fn {name}({params}){returns}");
        self.block(&header, &function.body, "");
    }

    /// Writes `header {`, the block's statements and `}` + `after`.
    fn block(&mut self, header: &str, block: &Block, after: &str) {
        let has_comments = self
            .comments
            .get(self.next_comment)
            .is_some_and(|c| c.start < block.span.end);

        if block.stmts.is_empty() && !has_comments {
            let line = format!("{header} {{}}{after}");
            self.line(&line, block.span.end);
            return;
        }

        let open = block.span.start + 1;
        self.line(&format!("{header} {{"), open);
        self.indent += 1;

        let mut previous_end = None;

        for stmt in &block.stmts {
            if previous_end.is_some() && self.blank_lines_before(self.first_text(stmt.span.start)) > 0 {
                self.out.push('\n');
            }
            self.comments_above(stmt.span.start);
            self.stmt(stmt);
            previous_end = Some(stmt.span.end);
        }

        self.leading_comments(block.span.end - 1);
        self.indent -= 1;
        self.line(&format!("}}{after}"), block.span.end);
    }

    /// Where the text before `pos` begins: the first comment directly
    /// above it, so blank lines are counted before any comments.
    fn first_text(&self, pos: usize) -> usize {
        match self.comments.get(self.next_comment) {
            Some(comment) if comment.start < pos => comment.start,
            _ => pos,
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        let end = stmt.span.end;

        match &stmt.kind {
            StmtKind::Decl(decl) => {
                let line = format!("{}: {}", decl.name.name, type_expr(&decl.ty));
                self.line(&line, end);
            }

            StmtKind::Assign { targets, values } => {
                let targets = targets.iter().map(|t| self.place(t)).collect::<Vec<_>>();
                let line = format!("{} = {}", targets.join(", "), self.exprs(values));
                self.line(&line, end);
            }

            StmtKind::Call(call) => {
                let line = self.call(call);
                self.line(&line, end);
            }

            StmtKind::Expr(expr) => {
                let line = self.expr(expr);
                self.line(&line, end);
            }

            StmtKind::Return(values) if values.is_empty() => self.line("return", end),

            StmtKind::Return(values) => {
                let line = format!("return {}", self.exprs(values));
                self.line(&line, end);
            }

            StmtKind::While { cond, body } => {
                let header = format!("while {}", self.expr(cond));
                self.block(&header, body, "");
            }

            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                let header = format!("if {}", self.expr(cond));

                match else_block {
                    None => self.block(&header, then_block, ""),
                    Some(else_block) => {
                        self.block(&header, then_block, " else {");
                        self.else_block(else_block);
                    }
                }
            }
        }
    }

    /// The statements of an `else` block, whose `} else {` line was already
    /// written.
    fn else_block(&mut self, block: &Block) {
        // `} else {` ended a line; drop that newline if the block is empty.
        if block.stmts.is_empty() && !self.comments_before(block.span.end) {
            self.out.truncate(self.out.len() - " else {\n".len());
            self.out.push_str(" else {}\n");
            return;
        }

        self.indent += 1;
        let mut first = true;

        for stmt in &block.stmts {
            if !first && self.blank_lines_before(self.first_text(stmt.span.start)) > 0 {
                self.out.push('\n');
            }
            self.comments_above(stmt.span.start);
            self.stmt(stmt);
            first = false;
        }

        self.leading_comments(block.span.end - 1);
        self.indent -= 1;
        self.line("}", block.span.end);
    }

    fn comments_before(&self, pos: usize) -> bool {
        self.comments
            .get(self.next_comment)
            .is_some_and(|c| c.start < pos)
    }

    fn place(&self, place: &Place) -> String {
        let mut out = place.name.name.clone();

        for elem in &place.path {
            match elem {
                PlaceElem::Index(index) => out += &format!("[{}]", self.expr(index)),
                PlaceElem::Field(field) => out += &format!(".{}", field.name),
            }
        }

        out
    }

    fn exprs(&self, exprs: &[Expr]) -> String {
        exprs.iter().map(|e| self.expr(e)).collect::<Vec<_>>().join(", ")
    }

    fn call(&self, call: &Call) -> String {
        format!("{}({})", call.callee.name, self.exprs(&call.args))
    }

    fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Int(value) => value.to_string(),
            ExprKind::Bool(value) => value.to_string(),
            ExprKind::Var(ident) => ident.name.clone(),
            ExprKind::Call(call) => self.call(call),
            ExprKind::Binary { op, lhs, rhs } => {
                format!("{} {} {}", self.expr(lhs), op.as_str(), self.expr(rhs))
            }
            ExprKind::ArrayList(elems) => format!("[{}]", self.exprs(elems)),
            ExprKind::ArrayRepeat(value, len) => format!("[{}; {len}]", self.expr(value)),
            ExprKind::Index { base, index } => {
                format!("{}[{}]", self.expr(base), self.expr(index))
            }
            ExprKind::Field { base, field } => format!("{}.{}", self.expr(base), field.name),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => format!("{}.{}({})", self.expr(receiver), method.name, self.exprs(args)),
            ExprKind::StructLit { name, fields } => {
                let fields = fields
                    .iter()
                    .map(|(field, value)| format!("{}: {}", field.name, self.expr(value)))
                    .collect::<Vec<_>>();
                format!("{}({})", name.name, fields.join(", "))
            }
        }
    }
}

fn type_expr(ty: &TypeExpr) -> String {
    ty.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, ir, syntax};

    fn fmt(source: &str) -> String {
        format(source, &syntax::parse(source).unwrap())
    }

    #[test]
    fn formats_a_messy_program() {
        let source = "type P struct {x:i32,y:i32}
fn  add(a:i32,b:i32):i32{return a+b}
fn main():i32{ p:P
p=P(x:1,y:2)
if p.x==1{return add(p.x,p.y)}else{return 0} }";

        assert_eq!(
            fmt(source),
            "type P struct {
  x: i32,
  y: i32,
}

fn add(a: i32, b: i32): i32 {
  return a + b
}

fn main(): i32 {
  p: P
  p = P(x: 1, y: 2)
  if p.x == 1 {
    return add(p.x, p.y)
  } else {
    return 0
  }
}
"
        );
    }

    #[test]
    fn keeps_comments_and_blank_lines() {
        let source = "// File comment.

// Adds.
fn add(a: i32, b: i32): i32 { // trailing
  x: i32

  // Before.
  x = a + b // after
  return x
}

type P struct {
  // The x.
  x: i32, // pixels
}

example add(1, 2) == 3
example add(0, 0) == 0
// The end.
";

        assert_eq!(
            fmt(source),
            "// File comment.

// Adds.
fn add(a: i32, b: i32): i32 { // trailing
  x: i32

  // Before.
  x = a + b // after
  return x
}

type P struct {
  // The x.
  x: i32, // pixels
}

example add(1, 2) == 3
example add(0, 0) == 0
// The end.
"
        );
    }

    #[test]
    fn short_forms() {
        assert_eq!(
            fmt("type E struct {} fn f() {} fn main(): i32 { if true {} else {} return 0 }"),
            "type E struct {}

fn f() {}

fn main(): i32 {
  if true {} else {}
  return 0
}
"
        );
    }

    /// Formatting every test program is stable and keeps its meaning.
    #[test]
    fn test_programs_round_trip() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test");

        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let source = std::fs::read_to_string(&path).unwrap();
            let formatted = fmt(&source);

            assert_eq!(fmt(&formatted), formatted, "{} is not stable", path.display());

            let ir = |source: &str| {
                let program = check::check(&syntax::parse(source).unwrap()).unwrap();
                ir::lower(&program).to_string()
            };
            assert_eq!(ir(&source), ir(&formatted), "{} changed meaning", path.display());

            let count = |s: &str| s.matches("//").count();
            assert_eq!(count(&source), count(&formatted), "{} lost comments", path.display());
        }
    }
}
