use std::{fmt::Debug, fmt::Write, hash::Hash, io, marker::PhantomData};

use dot::Dot;

use crate::{
    expression::{Cmp, Expression, ExpressionList, Literal, Sum},
    statement::{Assignment, If, Return, Statement},
    Block, Call, Function, Ident, Module, Variable,
};

impl_nodes! {
    Module => Module,
    Function => Function,
    Block => Block,
    Variable => Variable,
    Ident => Ident,
    Call => Call,

    // Statements
    Statement => Statement,
    Return => Return,
    Assignment => Assignment,
    If => If,

    // Expressions
    Expression => Expression,
    ExpressionList => ExpressionList,
    Literal => Literal,
    Cmp => Cmp,
    Sum => Sum
}

#[derive(Debug, Hash)]
pub struct ASTRef<T>
where
    T: ASTType,
{
    node_id: usize,
    _type: PhantomData<T>,
}

impl<T: ASTType> Dot for ASTRef<T> {
    fn dot(&self, _: &mut dyn io::Write) -> io::Result<String> {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.node_id).unwrap();

        Ok(label)
    }
}

impl<T: ASTType> Clone for ASTRef<T> {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id,
            _type: self._type,
        }
    }
}

impl<T: ASTType> Copy for ASTRef<T> {}

pub trait ASTType: Debug + Sized + Hash {
    fn get(pool: &AST, pool_ref: ASTRef<Self>) -> &Self;

    fn get_mut(pool: &mut AST, pool_ref: ASTRef<Self>) -> &mut Self;

    fn to_node(pool_ref: Self) -> ASTNode;

    fn pool_ref(pool_id: usize) -> ASTRef<Self> {
        ASTRef {
            node_id: pool_id,
            _type: PhantomData {},
        }
    }
}

#[derive(Debug)]
pub struct AST {
    pub path: String,
    pub input: String,
    pub id: usize,
    data: Vec<ASTNode>,
}

impl AST {
    pub fn new(path: String, input: String, id: usize) -> AST {
        AST {
            path,
            input,
            id,
            data: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn graph(&self, output: &mut dyn io::Write) -> io::Result<()> {
        writeln!(output, "digraph {{")?;

        for node in &self.data {
            node.dot(output)?;
        }

        writeln!(output, "}}")?;

        Ok(())
    }

    pub fn add<T>(&mut self, node: T) -> ASTRef<T>
    where
        T: ASTType + Debug,
    {
        self.data.push(T::to_node(node));

        T::pool_ref(self.data.len() - 1)
    }

    pub fn get<T>(&self, pool_ref: ASTRef<T>) -> &T
    where
        T: ASTType + Sized,
    {
        T::get(self, pool_ref)
    }

    pub fn get_mut<T>(&mut self, pool_ref: ASTRef<T>) -> &mut T
    where
        T: ASTType + Sized,
    {
        T::get_mut(self, pool_ref)
    }
}
