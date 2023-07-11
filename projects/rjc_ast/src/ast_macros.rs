macro_rules! impl_pool_type {
    ($enum:ident => $type:path) => {
        impl ASTType for $enum {
            fn get(pool: &AST, pool_ref: ASTRef<Self>) -> &Self {
                let data = &pool.data[pool_ref.pool_id];

                match data {
                    Node::$enum(node) => &node,

                    _ => panic!("tried to get wrong pool node type"),
                }
            }

            fn get_mut(pool: &mut AST, pool_ref: ASTRef<Self>) -> &mut Self {
                let data = &mut pool.data[pool_ref.pool_id];

                match data {
                    Node::$enum(node) => node,

                    _ => panic!("tried to get wrong pool node type"),
                }
            }

            fn to_node(pool_ref: Self) -> Node {
                Node::$enum(pool_ref)
            }
        }
    };
}

macro_rules! impl_nodes {
    {$($name:ident => $type:path),*} => {
        #[derive(Debug)]
        pub enum Node {
            $($name($type),)*
        }

        impl Node {
            fn dot(&self, output: &mut dyn io::Write) -> io::Result<()> {
                let node: &dyn Dot = match self {
                    $( Node::$name(node) => node, )*
                };

                node.dot(output)?;

                Ok(())
            }
        }

        $( impl_pool_type!($name => $type); )*
    };
}