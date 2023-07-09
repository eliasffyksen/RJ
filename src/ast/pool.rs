use std::{fmt::Debug, fmt::Write, io, marker::PhantomData, hash::Hash};

use super::*;
use dot::Dot;

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
        writeln!(output, "rankdir=\"LR\";")?;

        if self.data.len() == 0 {
            panic!("pool empty")
        }

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

#[derive(Debug)]
pub enum Node {
    Module(Module),
    Function(Function),
    Variable(Variable),
    Block(Block),
    Ident(Ident),

    Statement(statement::Statement),
    Return(statement::Return),
    Assignment(statement::Assignment),

    ExpressionList(expression::ExpressionList),
    Expression(expression::Expression),
    Literal(expression::Literal),
}

impl Node {
    fn dot(&self, output: &mut dyn io::Write) -> io::Result<()> {
        let node: &dyn Dot = match self {
            Node::Module(node) => node,
            Node::Function(node) => node,
            Node::Block(node) => node,
            Node::Variable(node) => node,
            Node::Ident(node) => node,

            Node::Statement(node) => node,
            Node::Return(node) => node,
            Node::Assignment(node) => node,

            Node::ExpressionList(node) => node,
            Node::Expression(node) => node,
            Node::Literal(node) => node,
        };

        node.dot(output)?;

        Ok(())
    }
}

macro_rules! impl_pool_type {
    ($enum:path, $type:path) => {
        impl PoolType for $type {
            fn get(pool: &Pool, pool_ref: PoolRef<Self>) -> &Self {
                let data = &pool.data[pool_ref.pool_id];

                match data {
                    $enum(node) => &node,

                    _ => panic!("tried to get wrong pool node type"),
                }
            }

            fn get_mut(pool: &mut Pool, pool_ref: PoolRef<Self>) -> &mut Self {
                let data = &mut pool.data[pool_ref.pool_id];

                match data {
                    $enum(node) => node,

                    _ => panic!("tried to get wrong pool node type"),
                }
            }

            fn to_node(pool_ref: Self) -> Node {
                $enum(pool_ref)
            }
        }
    };
}

impl_pool_type!(Node::Module, Module);
impl_pool_type!(Node::Function, Function);
impl_pool_type!(Node::Block, Block);
impl_pool_type!(Node::Variable, Variable);
impl_pool_type!(Node::Ident, Ident);

impl_pool_type!(Node::Statement, statement::Statement);
impl_pool_type!(Node::Return, statement::Return);
impl_pool_type!(Node::Assignment, statement::Assignment);

impl_pool_type!(Node::Expression, expression::Expression);
impl_pool_type!(Node::ExpressionList, expression::ExpressionList);
impl_pool_type!(Node::Literal, expression::Literal);
