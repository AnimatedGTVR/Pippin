# Pippin top-level convenience targets.
BUILD_DIR ?= build

.PHONY: all kernel run run-gdb iso clean distclean

all: kernel

kernel:
	cmake -S . -B $(BUILD_DIR) -G "Unix Makefiles"
	cmake --build $(BUILD_DIR) --target kernel.elf -j

run: kernel
	./scripts/run-qemu.sh

run-gdb: kernel
	./scripts/run-qemu.sh --gdb

iso:
	cmake -S . -B $(BUILD_DIR) -G "Unix Makefiles"
	cmake --build $(BUILD_DIR) --target kernel-limine.elf -j
	./scripts/make-iso.sh

clean:
	rm -rf $(BUILD_DIR)

distclean: clean
	rm -rf kernel/rust/target
