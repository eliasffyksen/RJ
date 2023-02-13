use std::collections::HashMap;

use crate::{stmt::VarDecl, ident::Ident};

pub trait Scopable {
    fn set_entry(&mut self, scope_entry: ScopeEntry);
    fn get_entry(&self, ident: &Ident) -> Option<&'_ ScopeEntry>;

    fn new_scope(&self) -> Scope<Self>
    where
        Self: Sized
    {
        Scope{
            entries: HashMap::new(),
            parent: self,
        }
    }
}

#[derive(Debug)]
pub struct NonScope {}

impl Scopable for NonScope {
    fn set_entry(&mut self, scope_entry: ScopeEntry) {
        panic!("Attempted to set entry {:?} in non scope", scope_entry);
    }

    fn get_entry(&self, _: &Ident) -> Option<&'_ ScopeEntry> {
        None
    }
}

#[derive(Debug)]
pub struct Scope<'parent, T> {
    entries: HashMap<Ident, ScopeEntry>,
    parent: &'parent T,
}

impl<T> Scopable for Scope<'_, T>
where
    T: Scopable
{
    fn set_entry(&mut self, scope_entry: ScopeEntry) {
        let key = &scope_entry.var_decl.ident;
        if self.entries.contains_key(key) {
            panic!("Attempted to set same variable in same scope twice: {:?}", scope_entry);
        }

        self.entries.insert(key.clone(), scope_entry);
    }

    fn get_entry(&self, ident: &Ident) -> Option<&ScopeEntry> {
        match self.entries.get(ident) {
            Some(scope_entry) => Some(scope_entry),
            None => self.parent.get_entry(ident),
        }
    }
}

#[derive(Debug)]
pub struct ScopeEntry {
    pub var_decl: VarDecl,
    pub register: usize,
}
