PYTHON = venv/bin/python
PROTOS = src/example.rs src/hangouts.rs

venv:
	python3 -m venv venv
	venv/bin/pip install protobuf

%.rs: %.proto codegen.py venv
	${PYTHON} codegen.py $< > $@

.PHONY: protos
protos: ${PROTOS}

.PHONY: test
test: protos
	RUST_BACKTRACE=1 cargo test

.PHONY: run
run: build venv
	RUST_BACKTRACE=1 ${PYTHON} libhangups.py

.PHONY: build
build: protos
	cargo build

.PHONY: check
check: protos
	cargo check

.PHONY: clean
clean:
	rm -rf target venv
