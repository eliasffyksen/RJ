use std::{marker::PhantomData, fmt::Debug, io, fmt::Write};

use super::*;
use dot::{Dot};

#[derive(Debug)]
pub struct PoolRef<T>
where
    T: PoolType + Debug,
{
    pool_id: usize,
    _type: PhantomData<T>,
}

impl<T: PoolType + Debug> Dot for PoolRef<T> {
    fn dot(&self, output: &mut dyn io::Write, label: &str) -> io::Result<()> {
        writeln!(output, "{} -> ast_node_{};", label, self.pool_id)?;

        Ok(())
    }
}

impl<T: PoolType + Debug> Clone for PoolRef<T> {
    fn clone(&self) -> Self {
        Self {
            pool_id: self.pool_id,
            _type: self._type,
        }
    }
}

impl<T: PoolType> Copy for PoolRef<T> {}

pub trait PoolType: Debug + Sized {
    fn get(pool: &Pool, pool_ref: PoolRef<Self>) -> &Self;

    fn get_mut(pool: &mut Pool, pool_ref: PoolRef<Self>) -> &mut Self;

    fn to_node(id: usize, pool_ref: Self) -> Node;

    fn pool_ref(pool_id: usize) -> PoolRef<Self>
    {
        PoolRef { pool_id, _type: PhantomData {} }
    }
}

macro_rules! impl_pool_type {
    ($enum:path, $type:path) => {
        impl PoolType for $type {
            fn get_mut(pool: &mut Pool, pool_ref: PoolRef<Self>) -> &mut Self
            {
                let data = &mut pool.data[pool_ref.pool_id];

                match data {
                    $enum(node) => &mut node.1,

                    _ => panic!("tried to get wrong pool node type"),
                }
            }

            fn get(pool: &Pool, pool_ref: PoolRef<Self>) -> &Self
            {
                let data = &pool.data[pool_ref.pool_id];

                match data {
                    $enum(node) => &node.1,

                    _ => panic!("tried to get wrong pool node type"),
                }
            }

            fn to_node(id: usize, pool_ref: Self) -> Node
            {
                $enum((id, pool_ref))
            }
        }
    };
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

    pub fn graph(&self, output: &mut dyn io::Write) -> io::Result<()> {
        writeln!(output, "digraph {{")?;
        writeln!(output, "rankdir=\"LR\";")?;

        if self.data.len() == 0 {
            panic!("pool empty")
        }

        for node in &self.data {
            node.dot(output)?;
        }

        writeln!(output)?;
        writeln!(output, "}}")?;

        Ok(())
    }

    pub fn add<T>(&mut self, node: T) -> PoolRef<T>
    where
        T: PoolType + Debug {
        self.data.push(T::to_node(self.data.len(), node));

        T::pool_ref(self.data.len() - 1)
    }

    pub fn get<T>(&self, pool_ref: PoolRef<T>) -> &T
    where
        T: PoolType + Sized
    {
        T::get(self, pool_ref)
    }

    pub fn get_mut<T>(&mut self, pool_ref: PoolRef<T>) -> &mut T
    where
        T: PoolType + Sized
    {
        T::get_mut(self, pool_ref)
    }
}

#[derive(Debug)]
pub enum Node {
    Function((usize, Function)),
    Variable((usize, Variable)),
    Block((usize, Block)),
    Statement((usize, statement::Statement)),
    Return((usize, statement::Return)),
    Module((usize, Module)),
    ExpressionList((usize, expression::ExpressionList)),
    Expression((usize, expression::Expression)),
    Constant((usize, expression::Literal)),
}

impl Node {
    fn get_id(&self) -> usize {
        *match self {
            Node::Function((id, _)) => id,
            Node::Variable((id, _)) => id,
            Node::Block((id, _)) => id,
            Node::Statement((id, _)) => id,
            Node::Return((id, _)) => id,
            Node::Module((id, _)) => id,
            Node::ExpressionList((id, _)) => id,
            Node::Expression((id, _)) => id,
            Node::Constant((id, _)) => id,
        }
    }

    fn dot(&self, output: &mut dyn io::Write) -> io::Result<()> {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.get_id()).unwrap();

        let node: &dyn Dot = match self {
            Node::Function((_, node)) => node,
            Node::Variable((_, node)) => node,
            Node::Block((_, node)) => node,
            Node::Statement((_, node)) => node,
            Node::Return((_, node)) => node,
            Node::Module((_, node)) => node,
            Node::ExpressionList((_, node)) => node,
            Node::Expression((_, node)) => node,
            Node::Constant((_, node)) => node,
        };

        node.dot(output, &label)?;

        Ok(())
    }
}

impl_pool_type!(Node::Function, Function);
impl_pool_type!(Node::Variable, Variable);
impl_pool_type!(Node::Block, Block);
impl_pool_type!(Node::Statement, statement::Statement);
impl_pool_type!(Node::Return, statement::Return);
impl_pool_type!(Node::Module, Module);
impl_pool_type!(Node::Constant, expression::Literal);
impl_pool_type!(Node::Expression, expression::Expression);
impl_pool_type!(Node::ExpressionList, expression::ExpressionList);
