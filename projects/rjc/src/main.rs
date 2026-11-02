use std::{fs, io, process::ExitCode};

mod check;
mod codegen;
mod config;
mod diagnostic;
mod hir;
mod ir;
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

    let program = match check::check(&module) {
        Ok(program) => program,
        Err(errors) => {
            for err in errors {
                eprint!("{}", err.render(&config.file_name, &source));
            }
            return ExitCode::FAILURE;
        }
    };

    if config.emit_hir {
        println!("{program:#?}");
    }

    let program = ir::lower(&program);

    if config.emit_ir {
        print!("{program}");
    }

    if config.emit_llvm {
        print!("{}", codegen::emit(&program, &config.file_name));
    }

    ExitCode::SUCCESS
}
