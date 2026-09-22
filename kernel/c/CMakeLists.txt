# C glue layer: libc-style stubs for the freestanding toolchain.
add_library(pippin_kernel_c STATIC
  src/klibc.c
  src/rust_eh.c
  src/shell_abi.c)

target_include_directories(pippin_kernel_c PUBLIC
  "${CMAKE_CURRENT_SOURCE_DIR}/include")