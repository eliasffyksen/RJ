use std::collections::HashMap;
use std::fs::read_to_string;

use pest::iterators::Pair;
use pest::error::Error;
use pest::Parser;

use crate::GenerateIR;
use crate::Rule;
use crate::function::Function;
use crate::RJParser;
use crate::GenerateAST;

#[derive(Debug, Default)]
pub struct File {
	name: String,
    functions: HashMap<String, Function>
}

impl File {
    pub fn read_file(filename: &'_ str) -> Result<File, Error<Rule>> {
		let input = read_to_string(filename).expect("Error reading file");
        let pair = RJParser::parse(Rule::file, input.as_str())?.next().unwrap();

		let mut file = File::generate_ast(pair);
		file.name = filename.to_string();

        Ok(file)
    }

	fn add_function(&mut self, function: Function) {
		let name = match &function.name {
			Some(name) => name.clone(),
            _ => panic!("Anonymous function not allowed in root: {:?}", function),
		};

		if self.functions.contains_key(&name) {
			panic!("Function name used twice in file: {}", name);
		}

		self.functions.insert(name, function);
	}
}

impl GenerateAST<File> for File {
    fn generate_ast(pair: Pair<Rule>) -> File {
        let mut file: File = Default::default();

        let inner = match pair.as_rule() {
            Rule::file => pair.into_inner(),

            _ => panic!("Trying to generate file from non file pair {:?}", pair),
        };

		for pair in inner {
			match pair.as_rule() {
				Rule::func => file.add_function(Function::generate_ast(pair)),
				Rule::EOI => (),

				_ => panic!("Invalid pair in file: {:?}", pair),
			}
		}

		file
    }
}

impl GenerateIR for File {
    fn generate_ir(&self, out: &mut impl std::io::Write) -> Result<(), std::io::Error> {
		writeln!(out, "source_filename = \"{}\"", self.name)?;
		writeln!(out)?;

		for (_, function) in &self.functions {
			function.generate_ir(out)?;
			writeln!(out)?;
		}

		Ok(())
    }
}
