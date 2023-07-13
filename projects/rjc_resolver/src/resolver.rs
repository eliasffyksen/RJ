use std::collections::HashMap;

use rjc_ast::{ASTRef, Call, Ident, AST};

#[derive(Clone, Copy)]
pub struct ResolutionRef {
    ast_id: usize,
    node_id: usize,
}

pub struct ASTResolution {
    data: HashMap<ASTRef<Ident>, ResolutionRef>,
}

pub struct Resolver {
    asts: Vec<AST>,
    resolutions: Vec<ASTResolution>,
}

impl Resolver {
    fn new(mut ast: AST) -> Resolver {
        ast.id = 0;

        Resolver {
            asts: vec![ast],
            resolutions: vec![ASTResolution{
                data: Default::default(),
            }],
        }
    }

    fn resolve(&mut self) {

    }
}
