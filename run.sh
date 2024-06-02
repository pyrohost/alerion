#!/bin/sh

docker container prune --force

if [ "$1" != "--no-clean" ]; then
	echo cleaning
	rm -r "$(jq -Mr '.data_dir' < $HOME/.config/alerion/config.json)"
fi

cargo build
if [ $? -eq 0 ]; then
	export RUST_LOG=debug
	export RUST_BACKTRACE=full
	target/debug/alerion_cli
fi
