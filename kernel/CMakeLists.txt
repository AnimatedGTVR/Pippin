# Kernel targets: share Rust, C++, and C across two boot ELFs.
add_subdirectory(c)
add_subdirectory(cpp)

add_executable(kernel.elf
  "${CMAKE_CURRENT_SOURCE_DIR}/asm/boot.S"
  "${CMAKE_CURRENT_SOURCE_DIR}/asm/isr.S")
target_sources(kernel.elf PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/asm/syscall.S")

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

# Direct Limine protocol image, entered in long mode at its high ELF entry.
add_executable(kernel-limine.elf
  "${CMAKE_CURRENT_SOURCE_DIR}/asm/isr.S"
  "${CMAKE_CURRENT_SOURCE_DIR}/asm/syscall.S"
  "${CMAKE_CURRENT_SOURCE_DIR}/cpp/src/limine.cc")
target_include_directories(kernel-limine.elf PRIVATE "${CMAKE_SOURCE_DIR}/kernel/c/include")
target_link_libraries(kernel-limine.elf PRIVATE
  -Wl,--start-group pippin_kernel_cpp pippin_kernel_c pippin_drivers
  "${PIPPIN_RUST_LIB}" -Wl,--end-group)
target_link_options(kernel-limine.elf PRIVATE
  -no-pie -nostdlib -Wl,--gc-sections -Wl,--build-id=none
  -Wl,-Map=${CMAKE_BINARY_DIR}/kernel-limine.map
  -T "${CMAKE_SOURCE_DIR}/kernel/limine-linker.ld")
add_dependencies(kernel-limine.elf rust-kernel)
set_target_properties(kernel-limine.elf PROPERTIES RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")
