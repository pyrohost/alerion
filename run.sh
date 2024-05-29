#!/bin/sh

cargo build
if [ $? ]; then
	export RUST_LOG=debug
	export RUST_BACKTRACE=full
	target/debug/alerion_cli
	docker container prune --force
	rm -r "$(jq -Mr '.data_dir' < $HOME/.config/alerion/config.json)"
fi
