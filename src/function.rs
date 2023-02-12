use pest::iterators::Pair;

use crate::{Rule, GenerateAST, GenerateIR};


#[derive(Debug, Default)]
pub struct Function {
		pub name: Option<String>
}

impl GenerateAST<Function> for Function {
	fn generate_ast(pair: Pair<Rule>) -> Function {
		let mut function: Function = Default::default();

		let inner = match pair.as_rule() {
			Rule::func => pair.into_inner(),
			_ => panic!("Trying to generate function from non function pair: {:?}", pair),
		};

		let name = inner
			.filter(|pair| pair.as_rule() == Rule::ident)
			.next();

		if let Some(name) = name {
			function.name = Some(name.as_span().as_str().to_string())
		}

		function
	}
}

impl GenerateIR for Function {
    fn generate_ir(&self, out: &mut impl std::io::Write) -> Result<(), std::io::Error> {
        let name = match &self.name {
            Some(name) => name,
            _ => panic!("Can not write LLVM IR for function without name: {:?}", self),
        };

        writeln!(out, "define void @{}() {{", name)?;
        writeln!(out, "  ret void")?;
        writeln!(out, "}}")?;

        Ok(())
    }
}
