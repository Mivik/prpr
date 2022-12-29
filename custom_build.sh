#!/bin/bash

packages="prpr prpr-client prpr-player prpr-render"

if [ "$1" = "c" ] ; then
	for i in $packages; do
		sed -Ei "s/# macroquad = \{ version = ([^,]+), default-features = false \}/macroquad = { version = \1, default-features = false }/" $i/Cargo.toml
		sed -Ei "s/# miniquad = \"\*\"/miniquad = \"*\"/" $i/Cargo.toml
		sed -Ei "s/macroquad = \{ path = \"..\/..\/macroquad\", default-features = false \}/# macroquad = { path = \"..\/..\/macroquad\", default-features = false }/" $i/Cargo.toml
		sed -Ei "s/miniquad = \{ path = \"..\/..\/miniquad\" \}/# miniquad = { path = \"..\/..\/miniquad\" }/" $i/Cargo.toml
	done
else
	for i in $packages; do
		sed -Ei "s/^macroquad = \{ version = ([^,]+), default-features = false \}/# macroquad = { version = \1, default-features = false }/" $i/Cargo.toml
		sed -Ei "s/^miniquad = \"\*\"/# miniquad = \"*\"/" $i/Cargo.toml
		sed -Ei "s/# macroquad = \{ path = \"..\/..\/macroquad\", default-features = false \}/macroquad = { path = \"..\/..\/macroquad\", default-features = false }/" $i/Cargo.toml
		sed -Ei "s/# miniquad = \{ path = \"..\/..\/miniquad\" \}/miniquad = { path = \"..\/..\/miniquad\" }/" $i/Cargo.toml
	done
fi
