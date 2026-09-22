set(PIPPIN_NATIVE_APP_DIR "${CMAKE_CURRENT_SOURCE_DIR}")
set(PIPPIN_HELLO_ELF "${CMAKE_BINARY_DIR}/apps/native/pippin_hello_app")
set(PIPPIN_APP_BLOB "${CMAKE_BINARY_DIR}/pippin_hello_blob.o")

add_executable(pippin_hello_app hello.cpp)
set_target_properties(pippin_hello_app PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/apps/native"
  OUTPUT_NAME "pippin-hello"
  SUFFIX ".elf")

target_compile_options(pippin_hello_app PRIVATE
  -ffreestanding
  -fno-pic
  -fno-pie
  -mcmodel=large
  -mno-red-zone
  -fno-stack-protector
  -fno-asynchronous-unwind-tables
  -ffunction-sections
  -fdata-sections
  -fno-exceptions
  -fno-rtti
  -fno-threadsafe-statics)

target_link_options(pippin_hello_app PRIVATE
  -nostdlib
  -no-pie
  -static
  -Wl,--gc-sections
  -Wl,--build-id=none
  -T "${CMAKE_CURRENT_SOURCE_DIR}/user.ld")

find_program(PIPPIN_OBJCOPY NAMES objcopy llvm-objcopy REQUIRED)

add_custom_command(
  OUTPUT "${PIPPIN_APP_BLOB}"
  COMMAND "${PIPPIN_OBJCOPY}"
          --input-target=binary
          --output-target=elf64-x86-64
          --binary-architecture=i386:x86-64
          --rename-section .data=.rodata.pippin_app,alloc,load,readonly,data,contents
          pippin-hello.elf "${PIPPIN_APP_BLOB}"
  COMMAND "${PIPPIN_OBJCOPY}"
          --redefine-sym _binary_pippin_hello_elf_start=pippin_hello_elf_start
          --redefine-sym _binary_pippin_hello_elf_end=pippin_hello_elf_end
          --redefine-sym _binary_pippin_hello_elf_size=pippin_hello_elf_size
          "${PIPPIN_APP_BLOB}"
  DEPENDS pippin_hello_app
  WORKING_DIRECTORY "${CMAKE_BINARY_DIR}/apps/native"
  COMMENT "Embedding native ELF app for Pippin loader"
  VERBATIM)

add_custom_target(pippin-native-app-blob DEPENDS "${PIPPIN_APP_BLOB}")
set(PIPPIN_APP_BLOB "${PIPPIN_APP_BLOB}" PARENT_SCOPE)
