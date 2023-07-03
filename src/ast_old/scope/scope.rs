use std::collections;

use crate::ast;
use crate::ast::stmt;

pub trait Scopable {
    fn set_ret_type(&mut self, ret_type: Vec<ast::Type>);
    fn get_ret_type(&self) -> Option<&'_ Vec<ast::Type>>;

    fn set_entry(&mut self, scope_entry: Entry);
    fn get_entry(&self, ident: &ast::Ident) -> Option<&'_ Entry>;

    fn new_scope(&self) -> Scope;
}

#[derive(Debug, Default)]
pub struct Scope<'a> {
    entries: collections::HashMap<String, Entry>,
    parent: Option<&'a Scope<'a>>,
    ret_type: Option<Vec<ast::Type>>,
}

impl Scopable for Scope<'_> {
    fn set_entry(&mut self, scope_entry: Entry) {
        let key = scope_entry.get_ident();
        if self.entries.contains_key(key) {
            panic!(
                "Attempted to set same variable in same scope twice: {:?}",
                scope_entry
            );
        }

        self.entries.insert(key.to_string(), scope_entry);
    }

    fn get_entry(&self, ident: &ast::Ident) -> Option<&Entry> {
        match self.entries.get(ident.get()) {
            Some(scope_entry) => Some(scope_entry),
            None => self.parent?.get_entry(ident),
        }
    }

    fn set_ret_type(&mut self, ret_type: Vec<ast::Type>) {
        self.ret_type = Some(ret_type);
    }

    fn get_ret_type(&self) -> Option<&'_ Vec<ast::Type>> {
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
pub struct Variable {
    pub var_decl: stmt::VarDecl,
    pub register: usize,
}

#[derive(Debug)]
pub struct Function {
    pub name: ast::Ident,
    pub args: Vec<ast::Type>,
    pub returns: Vec<ast::Type>,
}

#[derive(Debug)]
pub enum Entry {
    Variable(Variable),
    Function(Function),
}

impl Entry {
    fn get_ident(&self) -> &str {
        match self {
            Entry::Variable(variable) => variable.var_decl.ident.get(),
            Entry::Function(function) => function.name.get(),
        }
    }
}
