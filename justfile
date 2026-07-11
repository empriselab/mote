# Firmware recipes
mod firmware './mote-firmware'
# API recipes
mod api './mote-api'
# Documentation book recipes
mod book './mote-book'
# Configuration website recipes
mod config './mote-configuration'
# KiCAD circuit design recipes
mod hardware './mote-hardware'
# FFI recipes
mod ffi './mote-ffi'

[default]
_default:
    just --list

# Run the full CI suite
ci: firmware::ci api::ci book::ci config::ci ffi::ci

# Check spelling across the repo (matches the spelling CI job)
spell:
    codespell

# Check grammar and style in prose (matches the grammar CI job)
grammar:
    vale sync
    vale mote-book/src README.md mote-*/README.md

# Generate a folder for uploading to gh pages.
ci-web-artifact: book::build
    mkdir -p output
    cp -r mote-book/book/* output

