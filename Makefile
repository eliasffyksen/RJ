
./build/%.ll: example/%.rj
	cargo run -- --emit-llvm $^ > $@


./build/main: ./build/test.ll ./example/main.c
	clang -o $@ $^

build: ./build/main

run: build
	./build/main

clean:
	rm -fr ./build/*

.PHONY: build run clean
