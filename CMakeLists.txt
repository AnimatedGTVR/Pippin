# Pippin top-level build.
#
# Shared build inputs produce Multiboot and Limine kernel ELFs from a mix of
#   - Assembly  (kernel/asm/boot.S)  boot + long-mode bring-up
#   - Rust      (kernel/rust)        no_std staticlib, built by cargo
#   - C++       (kernel/cpp)         boot runtime / subsystem glue
#   - C         (kernel/c)           libc-style stubs
#   - C++       (drivers/cpp)        driver layer
# Each boot path is linked into a freestanding ELF by `ld`.
#
# The driver layer stays in this build until app-land exists; turn on
# PIPPIN_BUILD_APPS at Milestone 5 to add native C++ and Rust applications.
# Optional C# application support may follow on the x86-64 desktop.

cmake_minimum_required(VERSION 3.20)
project(Pippin VERSION 0.1.0 LANGUAGES ASM C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE OFF)

option(PIPPIN_BUILD_APPS "Build the future native application layer" OFF)

# ---- Freestanding codegen rules shared by every kernel/driver source ----
# -mcmodel=large: both kernel images link in the higher half,
# outside the ±2 GiB reach that small-model 32-bit relocations assume.
add_compile_options(
  -ffreestanding
  -fno-pic
  -fno-pie
  -mcmodel=large
  -mno-red-zone
  -fno-stack-protector
  -fno-omit-frame-pointer
  -fno-asynchronous-unwind-tables
  -ffunction-sections
  -fdata-sections
  -Wall
  -Wextra
)
add_compile_options($<$<COMPILE_LANGUAGE:CXX>:-fno-exceptions>)
add_compile_options($<$<COMPILE_LANGUAGE:CXX>:-fno-rtti>)
add_compile_options($<$<COMPILE_LANGUAGE:CXX>:-fno-threadsafe-statics>)

# Drive the assembler through the C compiler so .S files are preprocessed.
set(CMAKE_ASM_COMPILER "${CMAKE_C_COMPILER}")

# ---- Rust kernel core (cargo staticlib) ---------------------------------
# The cargo target dir is mirrored below build/ so the compiler.spawned
# archive stays inside the CMake tree. PIPPIN_RUST_TARGET_SUBDIR must match
# the `[build] target` in kernel/rust/.cargo/config.toml (single place each
# side), which is why the produced archive is triple-qualified.
set(PIPPIN_RUST_CRATE_DIR "${CMAKE_SOURCE_DIR}/kernel/rust")
set(PIPPIN_RUST_TARGET_DIR "${CMAKE_BINARY_DIR}/kernel/rust/target")
set(PIPPIN_RUST_TARGET_SUBDIR "x86_64-unknown-none")
set(PIPPIN_RUST_LIB "${PIPPIN_RUST_TARGET_DIR}/${PIPPIN_RUST_TARGET_SUBDIR}/release/libpippin_kernel.a")

add_custom_command(
  OUTPUT "${PIPPIN_RUST_LIB}"
  COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${PIPPIN_RUST_TARGET_DIR}
          cargo build --release --manifest-path ${PIPPIN_RUST_CRATE_DIR}/Cargo.toml
  DEPENDS
    "${PIPPIN_RUST_CRATE_DIR}/Cargo.toml"
    "${PIPPIN_RUST_CRATE_DIR}/src/lib.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/apic.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/cpu.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/ffi.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/gdt.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/heap.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/idt.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/interrupts.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/mem.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/mm.rs"
    "${PIPPIN_RUST_CRATE_DIR}/src/serial.rs"
  WORKING_DIRECTORY "${PIPPIN_RUST_CRATE_DIR}"
  COMMENT "Building Rust kernel core (staticlib)"
  VERBATIM)

add_custom_target(rust-kernel DEPENDS "${PIPPIN_RUST_LIB}")

add_subdirectory(kernel)
add_subdirectory(drivers/cpp)
