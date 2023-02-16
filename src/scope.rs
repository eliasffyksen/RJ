use std::collections::HashMap;

use crate::{
    ident::Ident,
    stmt::{Type, VarDecl},
};

pub trait Scopable {
    fn set_ret_type(&mut self, ret_type: Vec<Type>);
    fn get_ret_type(&self) -> &'_ Vec<Type>;

    fn set_entry(&mut self, scope_entry: ScopeEntry);
    fn get_entry(&self, ident: &Ident) -> Option<&'_ ScopeEntry>;

    fn new_scope(&self) -> Scope<Self>
    where
        Self: Sized,
    {
        Scope {
            entries: HashMap::new(),
            parent: self,
            ret_type: None,
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

    fn set_ret_type(&mut self, ret_type: Vec<Type>) {
        panic!("Attempted to set return type {:?} in non scope", ret_type);
    }

    fn get_ret_type(&self) -> &'_ Vec<Type> {
        panic!("Attempted to get return type from non scope");
    }
}

#[derive(Debug)]
pub struct Scope<'parent, T> {
    entries: HashMap<Ident, ScopeEntry>,
    parent: &'parent T,
    ret_type: Option<Vec<Type>>,
}

impl<T> Scopable for Scope<'_, T>
where
    T: Scopable,
{
    fn set_entry(&mut self, scope_entry: ScopeEntry) {
        let key = &scope_entry.var_decl.ident;
        if self.entries.contains_key(key) {
            panic!(
                "Attempted to set same variable in same scope twice: {:?}",
                scope_entry
            );
        }

        self.entries.insert(key.clone(), scope_entry);
    }

    fn get_entry(&self, ident: &Ident) -> Option<&ScopeEntry> {
        match self.entries.get(ident) {
            Some(scope_entry) => Some(scope_entry),
            None => self.parent.get_entry(ident),
        }
    }

    fn set_ret_type(&mut self, ret_type: Vec<Type>) {
        self.ret_type = Some(ret_type);
    }

    fn get_ret_type(&self) -> &'_ Vec<Type> {
        match &self.ret_type {
            Some(ret_type) => ret_type,
            None => self.parent.get_ret_type(),
        }
    }
}

#[derive(Debug)]
pub struct ScopeEntry {
    pub var_decl: VarDecl,
    pub register: usize,
}
