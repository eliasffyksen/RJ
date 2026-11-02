# RJ - work in progress programming language

Are you too smart for [golang](https://go.dev/), but too dumb for [rust](https://www.rust-lang.org/)?

Fear not, you are not alone!

RJ to the rescue - *The brand new hipster language no one asked for*
## Pipeline

The compiler is a single crate, `projects/rjc`, with one module per stage.

### Parser and AST (`src/syntax`)

`rj.pest` holds the grammar, `parser.rs` turns pest's parse tree into the AST in
`ast.rs`. The AST is a plain owned tree where every node carries the byte span it
was parsed from, so later stages can report errors at the right place.

You can print the AST by running:
```sh
cargo run -- <FILE> --emit-ast
```

To output a graph in dot language you can run:
```sh
cargo run -- <FILE> --emit-ast-graph
```

Here is an example on the following code:
```
fn fib(n: i32): i32 {
  if n < 3 {
    return 1
  }

  return fib(n - 1) + fib(n - 2)
}
```

`cargo run -- test.rj --emit-ast-graph | dot -Tsvg > images/fib-ast.svg`:

![AST from fib function](./images/fib-ast.svg)

### Resolver and type checker (`src/check.rs`)

Resolves names and checks types, producing the program in `src/hir.rs`: variables
become indices into each function's table of locals, calls point directly at the
function they call, and every expression carries its type.

It also checks that every variable is assigned before it is read, that functions
return a value on every path, and that `fn main(): i32` exists. All errors are
reported, not just the first. To print the result:
```sh
cargo run -- <FILE> --emit-hir
```

### RJ IR (`src/ir`)

Lowers the checked program into a control-flow graph of basic blocks over a flat
table of locals. Parameters, return values, variables and temporaries are all
locals, statements only ever write whole locals, and a call reads all its
arguments before writing its results, so `x = f(x)` becomes `call f(x) -> [x]`.

To print it:
```sh
cargo run -- <FILE> --emit-ir
```

A small interpreter (used by the tests) runs the IR directly, so lowering is
checked against every program in `test/` without needing LLVM.

### Optimiser

**Not implemented** Infers how each function uses its parameters (read only, in/out,
consumed) and turns value copies into in-place updates where the old value is no
longer needed.

### LLVM Generator (`src/codegen.rs`)

Emits textual LLVM IR from the RJ IR. Every function returns `void` and takes one
pointer per return value followed by its arguments by value; a call passes the
destination variables' storage as those pointers. Functions are emitted as
`rj_<name>`.

`stdlib/start.ll` provides `_start`, which calls `main` and exits with its return
value using the Linux x86-64 `exit_group` syscall, so programs need neither libc
nor a dynamic linker.

## Testing

```sh
cargo test   # unit tests, plus every program in test/ through the IR interpreter
             # and, if llc and ld.lld are installed, compiled to native code
make test    # compiles each test/*.rj with llc + ld.lld and runs it
```

Each program in `test/` passes by exiting with 0.
