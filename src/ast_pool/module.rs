use super::*;

pub struct Module {
    source: String,
    source_path: String,
    functions: Vec<PoolRef<Function>>,
}
