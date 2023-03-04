BUILD_PATH?=./build
RJ_DYNAMIC_LINKER?=/lib/ld-linux-x86-64.so.2
RJ_LD_LIBRARY_PATH?=/lib

TEST_SRC=$(wildcard test/*.rj)
TESTS=$(TEST_SRC:test/%.rj=%)
TEST_BINS=$(TESTS:%=$(BUILD_PATH)/test/%)

.PHONY: test
test: $(TESTS:%=test.%)

test.%: $(BUILD_PATH)/test/%
	$^

.PHONY: clean
clean:
	rm -fr ./build/*

$(BUILD_PATH)/stdlib/start.o: stdlib/start.ll
	mkdir -p $(dir $@)
	llc --filetype obj -o $@ $^

$(BUILD_PATH)/%.ll: %.rj
	mkdir -p $(dir $@)
	cargo run -- --emit-llvm $^ > $@

$(BUILD_PATH)/%.o: $(BUILD_PATH)/%.ll
	mkdir -p $(dir $@)
	llc --filetype obj -o $@ $^

$(BUILD_PATH)/%: $(BUILD_PATH)/%.o $(BUILD_PATH)/stdlib/start.o
	ld.lld --dynamic-linker $(RJ_DYNAMIC_LINKER) -L$(RJ_LD_LIBRARY_PATH) -lc -o $@ $^

