use std::{fs, io, process::ExitCode};

mod config;
mod diagnostic;
mod syntax;

fn main() -> ExitCode {
    let config = config::Config::new();

    let source = match fs::read_to_string(&config.file_name) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("{}: error: {err}", config.file_name);
            return ExitCode::FAILURE;
        }
    };

    let module = match syntax::parse(&source) {
        Ok(module) => module,
        Err(err) => {
            eprint!("{}", err.render(&config.file_name, &source));
            return ExitCode::FAILURE;
        }
    };

    if config.emit_ast {
        println!("{module:#?}");
    }

    if config.emit_ast_graph {
        if let Err(err) = syntax::graph::write(&module, &mut io::stdout().lock()) {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    }

    if config.emit_llvm {
        eprintln!("error: --emit-llvm is not implemented yet");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
