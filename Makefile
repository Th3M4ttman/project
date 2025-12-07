

build:
	cargo build

run:
	cargo build
	./target/debug/project

clean:
	rm -rf ./target/
	
install:
	cargo build
	cp ./target/debug/project ~/bin/project
