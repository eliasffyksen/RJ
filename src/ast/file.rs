use std::collections;
use std::fs;
use std::io;

use pest::error::Error;
use pest::iterators::Pair;

use crate::ast;
use crate::ast::scope;
use crate::ast::scope::Scopable;
use crate::parser;
use crate::parser::ParserTrait;

#[derive(Debug, Default)]
pub struct File {
    name: String,
    functions: collections::HashMap<String, ast::Func>,
    input: String,
}

impl File {
    pub fn read_file(filename: &'_ str) -> Result<File, Error<parser::Rule>> {
        let input = fs::read_to_string(filename).expect("Error reading file");
        let pair = parser::Parser::parse(parser::Rule::file, input.as_str())?
            .next()
            .unwrap();

        let mut file = File::ast(pair);
        file.input = input;
        file.name = filename.to_string();

        Ok(file)
    }

    fn add_function(&mut self, function: ast::Func) {
        let name = match &function.name {
            Some(name) => name.clone(),
            _ => panic!("Anonymous function not allowed in root: {:?}", function),
        };

        if self.functions.contains_key(name.get()) {
            panic!("Function name used twice in file: {}", name.get());
        }

        self.functions.insert(name.get().to_string(), function);
    }

    pub fn ir(&self, out: &mut impl io::Write, context: &mut ast::IRContext) -> Result<(), ()> {
        writeln!(out, "source_filename = \"{}\"", self.name).unwrap();
        writeln!(out).unwrap();

        let mut success = true;

        let mut scope: scope::Scope = Default::default();

        for (_, function) in &self.functions {
            let function_name = function.name.clone().unwrap();

            scope.set_entry(scope::Entry::Function(scope::Function {
                name: function_name,
                args: function
                    .args
                    .iter()
                    .map(|arg| arg.var_type.clone())
                    .collect(),
                returns: function.ret_type.iter().map(|t| t.clone()).collect(),
            }))
        }

        let scope = scope;

        for (_, function) in &self.functions {
            match function.ir(out, context, &scope) {
                Ok(_) => (),
                Err(err) => {
                    success = false;
                    err.display(&mut io::stderr(), &self.name, &self.input)
                },
            }
            writeln!(out).unwrap();
        }

        if !success {
            return Err(())
        }

        Ok(())
    }

    pub fn ast(pair: Pair<parser::Rule>) -> File {
        let mut file: File = Default::default();

        let inner = match pair.as_rule() {
            parser::Rule::file => pair.into_inner(),

            _ => panic!("Trying to generate file from non file pair {:?}", pair),
        };

        for pair in inner {
            match pair.as_rule() {
                parser::Rule::func => file.add_function(ast::Func::ast(pair)),
                parser::Rule::EOI => (),

                _ => panic!("Invalid pair in file: {:?}", pair),
            }
        }

        file
    }
}
