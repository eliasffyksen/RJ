# RJ roadmap

> The guarantees of a pure functional language, written like Go.

RJ is a small, total language with value semantics. Code inside a function
looks imperative; across functions everything is pure. The compiler does the
clever parts (in-place updates, termination, easy proofs) so that the
programmer, human or LLM, writes plain code and specifications, and the
compiler verifies them.

## Principles

1. **Mutation is notation, never semantics.** Variables can be updated inside
   a function. Across function boundaries everything is passed by value.
2. **Everything terminates.** Loops are bounded and there is no recursion, so
   every function is total by construction. Long-running behaviour lives in a
   small runtime shell outside the language.
3. **The compiler does the clever parts.** Copies become in-place updates
   (parameter modes, liveness), easy proof obligations are discharged
   automatically, and only real properties need specs.
4. **One way to do each thing; no hidden costs.** No overloading, macros,
   exceptions, null or laziness. Copies the compiler can't remove are
   reported.
5. **Feedback is a feature.** Diagnostics are precise, machine-readable and
   come with counterexamples, so verification is a tool call, not reasoning.

## Where we are

Working end to end (parse → check → IR → LLVM), with the `restructure`
branch's test suite (`cargo test`, `make test`):

- `i32`, `bool`, fixed-size arrays, `type` definitions (structs and named
  types), methods with `this`.
- `if`/`else`, `while`, multiple return values, parallel assignment,
  assignment to parts of variables (`g.nodes[i].value = v`).
- Definite assignment, return-on-all-paths and type checking with spanned
  errors.
- Parameter modes (read / consume / in/out) and liveness: `x = f(x)`,
  `g.nodes[i] = f(g.nodes[i])` and `g.nodes[i].bump()` compile with no
  copies.
- An IR interpreter used by the tests as a reference for codegen.

- Phase 1's feedback loop: error codes with `--explain`, `--json`
  diagnostics with help and expected/found types, `example`s run at compile
  time, `--copies` notes and `--fmt`.

Not yet: totality (`while` and recursion are unrestricted), sum types,
growable collections, strings, I/O, specs.

## Phases

Each phase is usable on its own. "Done when" is the bar for moving on.

### Phase 1: Feedback loop (done)

Make the compiler a good partner before adding proof machinery.

- `rjc check --json`: diagnostics with stable codes (`E0001`...), spans,
  expected/found types and suggested fixes. `rjc explain E0001`.
- `example f(2, 3) == 5` declarations, run by the IR interpreter as part of
  `rjc check`.
- Copy notes: report every copy codegen keeps, with the reason ("also read by
  this call", "still used after").
- Canonical formatter (`rjc fmt`).

**Done when** an LLM can fix a type error, a failing example and an avoidable
copy using only `--json` output.

### Phase 2: Total core

Termination by construction, with no proofs needed.

- `for i in a..b`, `for i in xs.indices()`, `for x in xs`: bounds evaluated
  once, loop variable immutable, `x` bound as a place (`xs[i]`) so elements
  are never copied out.
- `break` and `continue`; loops can count as returning (no more dead
  `return` after `while true`).
- Remove `while`; "until done" loops become `for _ in 0..max` with an
  explicit fallback.
- Reject recursion: the call graph must be acyclic.
- Port the test programs (`fib` becomes iterative, `first_over` a bounded
  search).

**Done when** every accepted program provably terminates and the copy-count
tests still pass.

### Phase 3: Algebraic data and errors as values

- Enums with payloads: `type Shape enum { Circle(r: i32), Rect(w: i32, h: i32) }`.
- Exhaustive `match`, binding payload fields as places.
- `Option` and `Result` in the standard library; `xs.get(i)` returns
  `Option`.

**Done when** a validation-style program (parse a record, report errors)
reads naturally without traps or sentinel values.

### Phase 4: Generics and collections

- Generic types and functions, compiled once per concrete type so parameter
  modes stay exact.
- `Vec<T>` on the heap with value semantics: handing over a buffer is a
  pointer move, copying is explicit in copy notes.
- `Map<K, V>` and `String`.
- Functions as arguments, compiled per call site with the function plugged
  in. No closures that capture mutable state.
- Projections (`proj node(g, id): Node { yield g.nodes[id] }`) for libraries
  that hide their layout.

**Done when** the graph library from the design discussions is written with
`Vec` and `NodeId`, and `g.add_edge(a, b)` in a loop copies nothing.

### Phase 5: Specs, checked by running

- `requires`, `ensures`, `invariant` (on `for`), `assert`; bounded
  quantifiers (`forall i in 0..len(xs): ...`). Specs are RJ expressions, and
  any function can be used in them.
- Check specs at run time in debug builds and by fuzzing: all functions are
  total and pure, so the interpreter can run any function on random inputs.
- Report one of three states per function: violated (with an input),
  tested, proved.
- Typed holes: `todo` reports the expected type, variables in scope and open
  obligations.

**Done when** a wrong implementation of a specified function is reported with
a concrete failing input in RJ syntax.

### Phase 6: Proofs

- Automatic obligations: indexes, overflow, division by zero.
  - Pattern-discharged first (`for i in xs.indices()` makes `xs[i]` safe),
    no solver involved.
  - Proven indexes drop their run-time bounds check.
- Verification conditions from the checked program to Z3 for the rest, kept
  to decidable fragments. Timeouts are "unknown" with a hint, never a silent
  failure.
- `ghost` functions and `lemma`s for proofs that need help.
- Wrapping arithmetic is explicit (`wrapping_add`); specs use mathematical
  integers.

**Done when** a ledger example (`transfer` preserves the total balance) is
proved automatically, and typical array code needs no annotations.

### Phase 7: Runtime shell

- Programs as state machines: `fn App.step(e: Event): [Command]`, with
  `this` as the state (in/out, so updates are in place).
- A small host runtime that delivers events and performs commands (time,
  files, network, database), the only non-total part of the system.
- Replace `stdlib/start.ll`'s exit-code-only `main` with this runtime.

**Done when** a small HTTP or CLI service runs with all of its logic total
and its specs proved.

## Later / open questions

- Exporting obligations to Lean for proofs that are too hard for SMT.
- Proving the compiler's copy elision preserves semantics.
- Distinguishing different elements of the same variable (`g.nodes[0]` vs
  `g.nodes[1]`) to remove the remaining copies.
- Whether recursion with a compiler-checked decreasing argument is worth
  allowing for tree-shaped problems (parsers, JSON), if arenas plus work
  stacks prove too clunky in practice.
- Building nested literals and returned locals directly in their destination.
