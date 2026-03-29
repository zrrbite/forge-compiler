#!/usr/bin/env bash
# Benchmark: recursive fibonacci(35) across languages.
# Measures compilation time, binary size, and runtime.

set -e
cd "$(dirname "$0")"

echo "=== Forge vs C vs Rust vs Zig: fib(35) ==="
echo ""

# ── Forge (compiled) ─────────────────────────────────────────────────
echo "--- Forge (compiled via LLVM) ---"
FORGE_COMPILE_START=$(date +%s%N)
cargo run --release --quiet --manifest-path ../Cargo.toml -- --compile fib.fg -o fib_forge 2>/dev/null
FORGE_COMPILE_END=$(date +%s%N)
FORGE_COMPILE_MS=$(( (FORGE_COMPILE_END - FORGE_COMPILE_START) / 1000000 ))
FORGE_SIZE=$(stat --printf="%s" fib_forge 2>/dev/null || stat -f%z fib_forge)
echo "  Compile: ${FORGE_COMPILE_MS}ms"
echo "  Size:    ${FORGE_SIZE} bytes"
FORGE_RUN_START=$(date +%s%N)
FORGE_OUT=$(./fib_forge)
FORGE_RUN_END=$(date +%s%N)
FORGE_RUN_MS=$(( (FORGE_RUN_END - FORGE_RUN_START) / 1000000 ))
echo "  Runtime: ${FORGE_RUN_MS}ms"
echo "  Output:  ${FORGE_OUT}"
echo ""

# ── Forge (interpreted) ──────────────────────────────────────────────
echo "--- Forge (interpreted) ---"
FORGE_INTERP_START=$(date +%s%N)
FORGE_INTERP_OUT=$(cargo run --release --quiet --manifest-path ../Cargo.toml -- fib.fg)
FORGE_INTERP_END=$(date +%s%N)
FORGE_INTERP_MS=$(( (FORGE_INTERP_END - FORGE_INTERP_START) / 1000000 ))
echo "  Runtime: ${FORGE_INTERP_MS}ms"
echo "  Output:  ${FORGE_INTERP_OUT}"
echo ""

# ── C (gcc -O2) ──────────────────────────────────────────────────────
echo "--- C (gcc -O2) ---"
C_COMPILE_START=$(date +%s%N)
gcc -O2 fib.c -o fib_c
C_COMPILE_END=$(date +%s%N)
C_COMPILE_MS=$(( (C_COMPILE_END - C_COMPILE_START) / 1000000 ))
C_SIZE=$(stat --printf="%s" fib_c 2>/dev/null || stat -f%z fib_c)
echo "  Compile: ${C_COMPILE_MS}ms"
echo "  Size:    ${C_SIZE} bytes"
C_RUN_START=$(date +%s%N)
C_OUT=$(./fib_c)
C_RUN_END=$(date +%s%N)
C_RUN_MS=$(( (C_RUN_END - C_RUN_START) / 1000000 ))
echo "  Runtime: ${C_RUN_MS}ms"
echo "  Output:  ${C_OUT}"
echo ""

# ── Rust (rustc -O) ──────────────────────────────────────────────────
echo "--- Rust (rustc -O) ---"
RUST_COMPILE_START=$(date +%s%N)
rustc -O fib.rs -o fib_rust
RUST_COMPILE_END=$(date +%s%N)
RUST_COMPILE_MS=$(( (RUST_COMPILE_END - RUST_COMPILE_START) / 1000000 ))
RUST_SIZE=$(stat --printf="%s" fib_rust 2>/dev/null || stat -f%z fib_rust)
echo "  Compile: ${RUST_COMPILE_MS}ms"
echo "  Size:    ${RUST_SIZE} bytes"
RUST_RUN_START=$(date +%s%N)
RUST_OUT=$(./fib_rust)
RUST_RUN_END=$(date +%s%N)
RUST_RUN_MS=$(( (RUST_RUN_END - RUST_RUN_START) / 1000000 ))
echo "  Runtime: ${RUST_RUN_MS}ms"
echo "  Output:  ${RUST_OUT}"
echo ""

# ── Zig (zig build-exe -OReleaseFast) ────────────────────────────────
if command -v zig &>/dev/null; then
    echo "--- Zig (ReleaseFast) ---"
    ZIG_COMPILE_START=$(date +%s%N)
    zig build-exe fib.zig -OReleaseFast -femit-bin=fib_zig 2>/dev/null
    ZIG_COMPILE_END=$(date +%s%N)
    ZIG_COMPILE_MS=$(( (ZIG_COMPILE_END - ZIG_COMPILE_START) / 1000000 ))
    ZIG_SIZE=$(stat --printf="%s" fib_zig 2>/dev/null || stat -f%z fib_zig)
    echo "  Compile: ${ZIG_COMPILE_MS}ms"
    echo "  Size:    ${ZIG_SIZE} bytes"
    ZIG_RUN_START=$(date +%s%N)
    ZIG_OUT=$(./fib_zig)
    ZIG_RUN_END=$(date +%s%N)
    ZIG_RUN_MS=$(( (ZIG_RUN_END - ZIG_RUN_START) / 1000000 ))
    echo "  Runtime: ${ZIG_RUN_MS}ms"
    echo "  Output:  ${ZIG_OUT}"
    echo ""
else
    echo "--- Zig: not installed, skipping ---"
    echo ""
fi

# ── Summary ──────────────────────────────────────────────────────────
echo "=== Summary ==="
printf "%-20s %10s %10s %10s\n" "Language" "Compile" "Runtime" "Size"
printf "%-20s %9sms %9sms %8s bytes\n" "Forge (compiled)" "$FORGE_COMPILE_MS" "$FORGE_RUN_MS" "$FORGE_SIZE"
printf "%-20s %10s %9sms %10s\n" "Forge (interpreted)" "N/A" "$FORGE_INTERP_MS" "N/A"
printf "%-20s %9sms %9sms %8s bytes\n" "C (gcc -O2)" "$C_COMPILE_MS" "$C_RUN_MS" "$C_SIZE"
printf "%-20s %9sms %9sms %8s bytes\n" "Rust (rustc -O)" "$RUST_COMPILE_MS" "$RUST_RUN_MS" "$RUST_SIZE"

# Clean up
rm -f fib_forge fib_c fib_rust fib_zig fib_zig.o
