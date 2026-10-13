#!/usr/bin/env bash
set -euo pipefail
controls_dir="$(cd "$(dirname "$0")/.." && pwd)"
test_dir="$(mktemp -d)"
trap 'test ! -f "$test_dir/controller-test" || rm "$test_dir/controller-test"; rmdir "$test_dir"' EXIT
"${CXX:-c++}" -std=c++17 -Wall -Wextra -Werror -fsanitize=address,undefined \
  -I "$controls_dir/include" "$controls_dir/tests/controller_test.cpp" -o "$test_dir/controller-test"
"$test_dir/controller-test"
