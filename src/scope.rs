use std::{collections::HashMap, default};

use crate::{
    ident::Ident,
    stmt::VarDecl,
    ast_type::Type,
};

pub trait Scopable {
    fn set_ret_type(&mut self, ret_type: Vec<Type>);
    fn get_ret_type(&self) -> Option<&'_ Vec<Type>>;


    fn set_entry(&mut self, scope_entry: ScopeEntry);
    fn get_entry(&self, ident: &Ident) -> Option<&'_ ScopeEntry>;

    fn new_scope(&self) -> Scope;
}

#[derive(Debug, Default)]
pub struct Scope<'a> {
    entries: HashMap<String, ScopeEntry>,
    parent: Option<&'a Scope<'a>>,
    ret_type: Option<Vec<Type>>,
}

impl Scopable for Scope<'_>
{
    fn set_entry(&mut self, scope_entry: ScopeEntry) {
        let key = scope_entry.get_ident();
        if self.entries.contains_key(key) {
            panic!(
                "Attempted to set same variable in same scope twice: {:?}",
                scope_entry
            );
        }

        self.entries.insert(key.to_string(), scope_entry);
    }

    fn get_entry(&self, ident: &Ident) -> Option<&ScopeEntry> {
        match self.entries.get(ident.get()) {
            Some(scope_entry) => Some(scope_entry),
            None => self.parent?.get_entry(ident),
        }
    }

    fn set_ret_type(&mut self, ret_type: Vec<Type>) {
        self.ret_type = Some(ret_type);
    }

    fn get_ret_type(&self) -> Option<&'_ Vec<Type>> {
        match &self.ret_type {
            Some(ret_type) => Some(ret_type),
            None => self.parent?.get_ret_type(),
        }
    }

    fn new_scope(&self) -> Scope<'_> {
        let mut new_scope: Scope = Default::default();
        new_scope.parent = Some(self);

        new_scope
    }
}

#[derive(Debug)]
pub struct ScopeVariable {
    pub var_decl: VarDecl,
    pub register: usize,
}

#[derive(Debug)]
pub struct ScopeFunction {
    pub name: Ident,
    pub args: Vec<Type>,
    pub returns: Vec<Type>,
}

#[derive(Debug)]
pub enum ScopeEntry {
    Variable(ScopeVariable),
    Function(ScopeFunction),
}

impl ScopeEntry {
    fn get_ident(&self) -> &str {
        match self {
            ScopeEntry::Variable(variable) => variable.var_decl.ident.get(),
            ScopeEntry::Function(function) => function.name.get(),
        }
    }
}
