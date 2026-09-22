# Pippin top-level convenience targets.
BUILD_DIR ?= build
QEMU_MODE ?= --defaultqemu

.PHONY: all preflight specs kernel run run-shell run-ui run-headless run-gdb run-disk disk iso clean distclean

all: kernel

preflight:
	BUILD_DIR="$(abspath $(BUILD_DIR))" bash scripts/preflight.sh

specs: preflight

kernel: preflight
	cmake -S . -B $(BUILD_DIR) -G "Unix Makefiles"
	bash -c 'source "$(BUILD_DIR)/pippin-limits.env"; cmake --build "$(BUILD_DIR)" --target kernel.elf -j"$$PIPPIN_BUILD_JOBS"'

run: kernel
	./scripts/run-qemu.sh $(QEMU_MODE)

run-shell: run

run-ui: run

run-headless: kernel
	./scripts/run-qemu.sh --headless

run-gdb: kernel
	./scripts/run-qemu.sh $(QEMU_MODE) --gdb

disk:
	./scripts/make-fat-image.sh

run-disk: kernel disk
	./scripts/run-qemu.sh $(QEMU_MODE) --disk

iso: preflight
	cmake -S . -B $(BUILD_DIR) -G "Unix Makefiles"
	bash -c 'source "$(BUILD_DIR)/pippin-limits.env"; cmake --build "$(BUILD_DIR)" --target kernel-limine.elf -j"$$PIPPIN_BUILD_JOBS"'
	./scripts/make-iso.sh

clean:
	rm -rf $(BUILD_DIR)

distclean: clean
	rm -rf kernel/rust/target
