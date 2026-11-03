use argparse;

#[derive(Default)]
pub struct Config {
    pub emit_ast: bool,
    pub emit_ast_graph: bool,
    pub emit_hir: bool,
    pub emit_ir: bool,
    pub emit_llvm: bool,
    /// Report diagnostics as JSON on stdout.
    pub json: bool,
    /// Print the explanation of an error code and exit.
    pub explain: String,
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

            ap.refer(&mut config.emit_hir).add_option(
                &["--emit-hir"],
                argparse::StoreTrue,
                "Emit the resolved and type-checked program",
            );

            ap.refer(&mut config.emit_ir).add_option(
                &["--emit-ir"],
                argparse::StoreTrue,
                "Emit RJ IR",
            );

            ap.refer(&mut config.json).add_option(
                &["--json"],
                argparse::StoreTrue,
                "Report diagnostics as JSON on stdout",
            );

            ap.refer(&mut config.explain).add_option(
                &["--explain"],
                argparse::Store,
                "Explain an error code, like E0200",
            );

            ap.refer(&mut config.file_name)
                .add_argument("file", argparse::Store, "File to compile");

            ap.parse_args_or_exit();
        }

        return config;
    }
}
