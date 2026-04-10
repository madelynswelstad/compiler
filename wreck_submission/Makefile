PROGRAM=WRECK

all:
	cargo build --release
	cp target/release/$(PROGRAM) .
	chmod +x $(PROGRAM)
clean:
	cargo clean
	rm -f $(PROGRAM)
