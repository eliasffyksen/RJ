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
pub struct PoolRef<T>
where
    T: PoolType,
{
    pool_id: usize,
    _type: PhantomData<T>,
}

impl<T: PoolType> Dot for PoolRef<T> {
    fn dot(&self, _: &mut dyn io::Write) -> io::Result<String> {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.pool_id).unwrap();

        Ok(label)
    }
}

impl<T: PoolType> Clone for PoolRef<T> {
    fn clone(&self) -> Self {
        Self {
            pool_id: self.pool_id,
            _type: self._type,
        }
    }
}

impl<T: PoolType> Copy for PoolRef<T> {}

pub trait PoolType: Debug + Sized + Hash {
    fn get(pool: &Pool, pool_ref: PoolRef<Self>) -> &Self;

    fn get_mut(pool: &mut Pool, pool_ref: PoolRef<Self>) -> &mut Self;

    fn to_node(pool_ref: Self) -> Node;

    fn pool_ref(pool_id: usize) -> PoolRef<Self> {
        PoolRef {
            pool_id,
            _type: PhantomData {},
        }
    }
}

#[derive(Debug)]
pub struct Pool {
    pub path: String,
    pub input: String,
    data: Vec<Node>,
}

impl Pool {
    pub fn new(path: String, input: String) -> Pool {
        Pool {
            path,
            input,
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

    pub fn add<T>(&mut self, node: T) -> PoolRef<T>
    where
        T: PoolType + Debug,
    {
        self.data.push(T::to_node(node));

        T::pool_ref(self.data.len() - 1)
    }

    pub fn get<T>(&self, pool_ref: PoolRef<T>) -> &T
    where
        T: PoolType + Sized,
    {
        T::get(self, pool_ref)
    }

    pub fn get_mut<T>(&mut self, pool_ref: PoolRef<T>) -> &mut T
    where
        T: PoolType + Sized,
    {
        T::get_mut(self, pool_ref)
    }
}
