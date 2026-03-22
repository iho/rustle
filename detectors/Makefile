.PHONY: pass rust-pass clean rust-clean

pass: $(patsubst %.cpp, %.so, $(filter-out near_core.cpp, $(wildcard *.cpp)))

%.so: %.cpp near_core.cpp near_core.h
	$(LLVM_CLANG) ${CXXFLAGS} $< near_core.cpp -o $@ ${LDFLAGS}

# Build all Rust detector binaries (output: target/release/<name>)
rust-pass:
	LLVM_SYS_211_PREFIX=$(LLVM_DIR) cargo build --release --manifest-path $(CURDIR)/Cargo.toml

clean:
	rm -rf *.so

rust-clean:
	cargo clean --manifest-path $(CURDIR)/Cargo.toml
