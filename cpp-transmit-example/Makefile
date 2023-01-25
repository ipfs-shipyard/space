all: ../target/debug/libtransmit.a
	mkdir -p build
	g++ src/main.cpp -L ../target/debug/ -ltransmit -o build/transmit
	
run:
	LD_LIBRARY_PATH=../target/debug/ ./build/transmit

../target/debug/libtransmit.a: src/lib.rs Cargo.toml
	cargo build

clean:
	rm -rf build && cargo clean