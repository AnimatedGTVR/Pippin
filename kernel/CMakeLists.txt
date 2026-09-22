# Kernel target: links the Assembly, Rust, C++ and C pieces into one ELF.
add_subdirectory(c)
add_subdirectory(cpp)

add_executable(kernel.elf
  "${CMAKE_CURRENT_SOURCE_DIR}/asm/boot.S"
  "${CMAKE_CURRENT_SOURCE_DIR}/asm/isr.S")

# Archives reference each other across the language boundary (e.g. Rust
# calls pippin_driver_count while C++ calls pippin_core_main), so link them
# in a single group to let ld rescan until all symbols resolve.
target_link_libraries(kernel.elf PRIVATE
  -Wl,--start-group
    pippin_kernel_cpp   # C++ boot runtime / subsystem glue
    pippin_kernel_c     # C glue: libc-style stubs used by C++ and Rust
    pippin_drivers      # driver layer (currently a stub)
    "${PIPPIN_RUST_LIB}"  # Rust kernel core (staticlib)
  -Wl,--end-group
)

target_link_options(kernel.elf PRIVATE
  -no-pie             # must be a plain ET_EXEC for the Multiboot loader
  -nostdlib           # freestanding: no libc/libgcc during the final link
  -Wl,--gc-sections   # drop unused sections (paired with -ffunction-sections)
  -Wl,--build-id=none # linker script discards .note.* sections anyway
  -Wl,-Map=${CMAKE_BINARY_DIR}/kernel.map
  -T "${CMAKE_SOURCE_DIR}/kernel/linker.ld"
)

# The kernel ELF depends on cargo having produced the Rust archive.
add_dependencies(kernel.elf rust-kernel)

# Place the final artifact at build/kernel.elf (scripts/Makefile expect it).
set_target_properties(kernel.elf PROPERTIES RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")