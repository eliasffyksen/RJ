# RJ - work in progress programming language

Are you too smart for [golang](https://go.dev/), but too dumb for [rust](https://www.rust-lang.org/)?

Fear not, you are not alone!

RJ to the rescue - *The brand new hipster language no one asked for*

## Current status

### The good news

It successfully compiles to to LLVM IR.

file `example/main.rj`:
```
fn main() {

}

fn test() {

}
```

Compiled with `cargo run example/main.rj` compiles to:
```
source_filename = "example/main.rj"

define void @test() {
  ret void
}

define void @main() {
  ret void
}
```

### Bad news

Currently it only support function definitions, so good luck
writing the next hottest tinder clone in this (almost) turing complete
language.
