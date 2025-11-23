

build:
	cargo build

run:
	cargo build
	./target/debug/project

install:
	cargo build
	cp ./target/debug/project ~/bin/project
