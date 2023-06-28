use std::marker::PhantomData;

use super::*;

pub struct PoolRef<T>
where
    T: PoolRefGet,
{
    pool_id: usize,
    _type: PhantomData<T>,
}

impl<T: PoolRefGet> Clone for PoolRef<T> {
    fn clone(&self) -> Self {
        Self {
            pool_id: self.pool_id,
            _type: self._type,
        }
    }
}

impl<T: PoolRefGet> Copy for PoolRef<T> {}

impl<T: PoolRefGet> PoolRef<T> {
    pub fn get(self, pool: &mut Pool) -> &mut T {
        T::get(pool, self)
    }
}

pub trait PoolRefGet {
    fn get(pool: &mut Pool, pool_ref: PoolRef<Self>) -> &mut Self
    where
        Self: Sized;
}

macro_rules! impl_pool_ref_get {
    ($enum:path, $type:path) => {
        impl PoolRefGet for $type {
            fn get(pool: &mut Pool, pool_ref: PoolRef<Self>) -> &mut Self
            where
                Self: Sized,
            {
                let data = &mut pool.data[pool_ref.pool_id];

                match data {
                    $enum(node) => node,

                    _ => panic!("tried to get wrong pool node type"),
                }
            }
        }
    };
}

pub struct Pool {
    data: Vec<Node>,
}

pub enum Node {
    Function(Function),
    Variable(Variable),
    Block(Block),
    Statement(Statement),
    Module(Module),
}

impl_pool_ref_get!(Node::Function, Function);
impl_pool_ref_get!(Node::Variable, Variable);
impl_pool_ref_get!(Node::Block, Block);
impl_pool_ref_get!(Node::Statement, Statement);
impl_pool_ref_get!(Node::Module, Module);
