use std::collections::HashMap;

use rjc_ast::Symbol;

use crate::resolver::ResolutionRef;

pub struct ErrorAlreadyDefined {
    earlier_definition: ResolutionRef,
}

pub struct Scope {
    parent: Option<Box<Scope>>,
    entries: HashMap<String, ResolutionRef>,
}

impl Scope {
    pub fn new() -> Scope {
        Scope {
            parent: None,
            entries: Default::default(),
        }
    }

    pub fn add(
        &mut self,
        identifier: &str,
        resolution: ResolutionRef,
    ) -> Result<(), ErrorAlreadyDefined> {
        if let Some(earlier_definition) = self.entries.get(identifier) {
            return Err(ErrorAlreadyDefined {
                earlier_definition: *earlier_definition,
            });
        }

        self.entries.insert(identifier.to_string(), resolution);

        Ok(())
    }

    pub fn lookup(&self, identifier: &str) -> Option<&ResolutionRef> {
        if let Some(resolution) = self.entries.get(identifier) {
            return Some(resolution);
        }

        match &self.parent {
            None => None,
            Some(parent) => parent.lookup(identifier),
        }
    }
}
