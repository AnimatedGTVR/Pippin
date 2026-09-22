# C++ kernel runtime: boot entry and subsystem glue.
add_library(pippin_kernel_cpp STATIC
  src/entry.cc
  src/shell.cc)

target_include_directories(pippin_kernel_cpp PUBLIC
  "${CMAKE_CURRENT_SOURCE_DIR}/include"
  "${CMAKE_SOURCE_DIR}/kernel/c/include")