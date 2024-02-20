CC=cargo
FMT=fmt

OPTIONS=

default: fmt
	$(CC) build
	@make clippy

fmt:
	$(CC) fmt --all

check:
	$(CC) test --all -- --show-output

example:
	@echo "No example for the moment"

clean:
	$(CC) clean

clippy:
	# $(CC) clippy --all --tests

coffee:
	$(CC) build --release
