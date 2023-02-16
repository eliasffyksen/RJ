# RJ - work in progress programming language

Are you too smart for [golang](https://go.dev/), but too dumb for [rust](https://www.rust-lang.org/)?

Fear not, you are not alone!

RJ to the rescue - *The brand new hipster language no one asked for*

## Current status

### The good news

It successfully compiles to to LLVM IR.

file `example/main.rj`:
```
fn test(a: i32): i32, i32, i32 {
  b: i32
  b = 666
  return a, b, 123
}

```

Compiled with `cargo run example/main.rj --emit-llvm` compiles to:
```
source_filename = "example/test.rj"

define void @test(i32* %0, i32* %1, i32* %2, i32 %3) {
  %5 = alloca i32
  store i32 %3, i32* %5
  %6 = alloca i32

  store i32 666, i32* %6

  %7 = load i32, i32* %5
  store i32 %7, i32* %0
  %8 = load i32, i32* %6
  store i32 %8, i32* %1
  store i32 123, i32* %2

  ret void
}
```

### Bad news

Currently it only support function definitions and variable declarations, returns, and
constant assignments, so good luck writing the next hottest tinder clone in this
(almost) turing complete language.
