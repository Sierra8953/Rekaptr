#!/bin/bash
rustc --edition=2021 src/ui/dashboard.rs -L dependency=target/debug/deps --crate-type=lib
