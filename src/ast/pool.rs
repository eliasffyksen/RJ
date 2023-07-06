use std::{marker::PhantomData, fmt::Debug};

use super::*;

#[derive(Debug)]
pub struct PoolRef<T>
where
    T: PoolType + Debug,
{
    pool_id: usize,
    _type: PhantomData<T>,
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

    fn to_node(pool_ref: Self) -> Node;

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
                    $enum(node) => node,

                    _ => panic!("tried to get wrong pool node type"),
                }
            }

            fn get(pool: &Pool, pool_ref: PoolRef<Self>) -> &Self
            {
                let data = &pool.data[pool_ref.pool_id];

                match data {
                    $enum(node) => node,

                    _ => panic!("tried to get wrong pool node type"),
                }
            }

            fn to_node(pool_ref: Self) -> Node
            {
                $enum(pool_ref)
            }
        }
    };
}

#[derive(Debug)]
pub struct Pool {
    data: Vec<Node>,
}

impl Pool {
    pub fn new() -> Pool {
        Pool { data: Vec::new() }
    }

    pub fn add<T>(&mut self, node: T) -> PoolRef<T>
    where
        T: PoolType + Debug {
        self.data.push(T::to_node(node));

        T::pool_ref(self.data.len() - 1)
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
    Function(Function),
    Variable(Variable),
    Block(Block),
    Statement(statement::Statement),
    Return(statement::Return),
    Module(Module),
    Expression(expression::Expression),
    ExpressionList(expression::List),
}

impl_pool_type!(Node::Function, Function);
impl_pool_type!(Node::Variable, Variable);
impl_pool_type!(Node::Block, Block);
impl_pool_type!(Node::Statement, statement::Statement);
impl_pool_type!(Node::Return, statement::Return);
impl_pool_type!(Node::Module, Module);
impl_pool_type!(Node::Expression, expression::Expression);
impl_pool_type!(Node::ExpressionList, expression::List);
