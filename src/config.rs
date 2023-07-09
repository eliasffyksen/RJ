use argparse;

#[derive(Default)]
pub struct Config {
    pub emit_ast: bool,
    pub emit_ast_graph: bool,
    pub emit_llvm: bool,
    pub file_name: String,
}

impl Config {
    pub fn new() -> Config {
        let mut config: Config = Default::default();

        {
            let mut ap = argparse::ArgumentParser::new();

            ap.refer(&mut config.emit_llvm).add_option(
                &["--emit-llvm"],
                argparse::StoreTrue,
                "Emit LLVM IR",
            );

            ap.refer(&mut config.emit_ast).add_option(
                &["--emit-ast"],
                argparse::StoreTrue,
                "Emit ast",
            );

            ap.refer(&mut config.emit_ast_graph).add_option(
                &["--emit-ast-graph"],
                argparse::StoreTrue,
                "Emit ast in graphviz form",
            );

            ap.refer(&mut config.file_name)
                .add_argument("file", argparse::Store, "File to parse")
                .required();

            ap.parse_args_or_exit();
        }

        return config;
    }
}
