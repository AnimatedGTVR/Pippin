# Driver layer (C++). Each driver registers with the Driver Manager; the
# registry grows at Milestone 3 (ACPI, PCI, PS/2, framebuffer, disk).
add_library(pippin_drivers STATIC
  src/pci.cc)

target_include_directories(pippin_drivers PUBLIC
  "${CMAKE_CURRENT_SOURCE_DIR}/include")