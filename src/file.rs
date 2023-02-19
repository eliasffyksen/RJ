use std::collections::HashMap;
use std::fs::read_to_string;
use std::io;

use pest::error::Error;
use pest::iterators::Pair;
use pest::Parser;

use crate::IRContext;
use crate::function::Function;
use crate::RJParser;
use crate::Rule;
use crate::scope::NonScope;
use crate::scope::Scopable;
use crate::scope::ScopeEntry;
use crate::scope::ScopeFunction;

#[derive(Debug, Default)]
pub struct File {
    name: String,
    functions: HashMap<String, Function>,
    input: String,
}

impl File {
    pub fn read_file(filename: &'_ str) -> Result<File, Error<Rule>> {
        let input = read_to_string(filename).expect("Error reading file");
        let pair = RJParser::parse(Rule::file, input.as_str())?.next().unwrap();

        let mut file = File::ast(pair);
        file.input = input;
        file.name = filename.to_string();

        Ok(file)
    }

    fn add_function(&mut self, function: Function) {
        let name = match &function.name {
            Some(name) => name.clone(),
            _ => panic!("Anonymous function not allowed in root: {:?}", function),
        };

        if self.functions.contains_key(name.get()) {
            panic!("Function name used twice in file: {}", name.get());
        }

        self.functions.insert(name.get().to_string(), function);
    }

    pub fn ir(&self, out: &mut impl std::io::Write, context: &mut IRContext) {
        writeln!(out, "source_filename = \"{}\"", self.name).unwrap();
        writeln!(out).unwrap();

        let mut scope = NonScope{}.new_scope();

        for (_, function) in &self.functions {
            let function_name = function.name.clone().unwrap();

            scope.set_entry(ScopeEntry::Function(ScopeFunction{
                name: function_name,
                args: function.args.iter().map(|arg| arg.var_type.clone()).collect(),
                returns: function.ret_type.iter().map(|t| t.clone()).collect(),
            }))
        }

        let scope = scope;

        for (_, function) in &self.functions {
            match function.ir(out, context, &scope) {
                Ok(_) => (),
                Err(err) => {
                    err.display(&mut io::stderr(), &self.name, &self.input)
                },
            }
            writeln!(out).unwrap();
        }
    }

    pub fn ast(pair: Pair<Rule>) -> File {
        let mut file: File = Default::default();

        let inner = match pair.as_rule() {
            Rule::file => pair.into_inner(),

            _ => panic!("Trying to generate file from non file pair {:?}", pair),
        };

        for pair in inner {
            match pair.as_rule() {
                Rule::func => file.add_function(Function::ast(pair)),
                Rule::EOI => (),

                _ => panic!("Invalid pair in file: {:?}", pair),
            }
        }

        file
    }
}
