set(PIPPIN_HELLO_ELF "${CMAKE_BINARY_DIR}/apps/native/pippin-hello.elf")

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

configure_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/embed.S.in"
  "${CMAKE_CURRENT_BINARY_DIR}/embed.S"
  @ONLY)

add_library(pippin_app_blob STATIC
  "${CMAKE_CURRENT_BINARY_DIR}/embed.S")
add_dependencies(pippin_app_blob pippin_hello_app)
