use std::{fs, io, process::ExitCode};

use diagnostic::{Code, Error};

mod check;
mod codegen;
mod config;
mod diagnostic;
mod examples;
mod hir;
mod ir;
mod syntax;
mod types;

fn main() -> ExitCode {
    let config = config::Config::new();

    if !config.explain.is_empty() {
        return explain(&config.explain);
    }

    if config.file_name.is_empty() {
        eprintln!("error: no input file (see --help)");
        return ExitCode::FAILURE;
    }

    let source = match fs::read_to_string(&config.file_name) {
        Ok(source) => source,
        Err(err) => {
            if config.json {
                println!("{{\"diagnostics\":[],\"error\":\"cannot read {}: {err}\"}}", config.file_name);
            } else {
                eprintln!("{}: error: {err}", config.file_name);
            }
            return ExitCode::FAILURE;
        }
    };

    let report = |errors: &[Error]| {
        if config.json {
            println!("{}", diagnostic::to_json(errors, &config.file_name, &source));
        } else {
            for err in errors {
                eprint!("{}", err.render(&config.file_name, &source));
            }
        }
    };

    let module = match syntax::parse(&source) {
        Ok(module) => module,
        Err(err) => {
            report(&[err]);
            return ExitCode::FAILURE;
        }
    };

    if config.fmt {
        let formatted = syntax::format::format(&source, &module);

        if !config.write {
            print!("{formatted}");
        } else if formatted != source {
            if let Err(err) = fs::write(&config.file_name, formatted) {
                eprintln!("{}: error: {err}", config.file_name);
                return ExitCode::FAILURE;
            }
        }

        return ExitCode::SUCCESS;
    }

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
            report(&errors);
            return ExitCode::FAILURE;
        }
    };

    if config.emit_hir {
        println!("{program:#?}");
    }

    let program = ir::lower(&program);

    let failed = examples::run(&program);
    if !failed.is_empty() {
        report(&failed);
        return ExitCode::FAILURE;
    }

    let notes = if config.copies {
        codegen::copy_notes(&program)
    } else {
        vec![]
    };

    if config.json || !notes.is_empty() {
        report(&notes);
    }

    if config.emit_ir {
        print!("{program}");
    }

    if config.emit_llvm {
        print!("{}", codegen::emit(&program, &config.file_name));
    }

    ExitCode::SUCCESS
}

fn explain(id: &str) -> ExitCode {
    match Code::from_id(id) {
        Some(code) => {
            println!("{}: {}\n\n{}", code.id(), code.title(), code.explanation());
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("error: unknown error code `{id}`");
            ExitCode::FAILURE
        }
    }
}
