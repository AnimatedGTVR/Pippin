# Pippin top-level convenience targets.
BUILD_DIR ?= build

.PHONY: all kernel run run-shell run-ui run-headless run-gdb run-disk disk iso clean distclean

all: kernel

kernel:
	cmake -S . -B $(BUILD_DIR) -G "Unix Makefiles"
	cmake --build $(BUILD_DIR) --target kernel.elf -j

run: kernel
	dotnet build apps/csharp/Pippin.Shell/Pippin.Shell.csproj
	./scripts/run-qemu.sh --shell

run-shell: run

run-ui: kernel
	dotnet build apps/csharp/Pippin.Broker/Pippin.Broker.csproj
	dotnet build apps/csharp/Pippin.Examples/Pippin.Examples.csproj
	./scripts/run-ui.sh

run-headless: kernel
	./scripts/run-qemu.sh --headless

run-gdb: kernel
	./scripts/run-qemu.sh --gdb

disk:
	./scripts/make-fat-image.sh

run-disk: kernel disk
	./scripts/run-qemu.sh --disk

iso:
	cmake -S . -B $(BUILD_DIR) -G "Unix Makefiles"
	cmake --build $(BUILD_DIR) --target kernel-limine.elf -j
	./scripts/make-iso.sh

clean:
	rm -rf $(BUILD_DIR)

distclean: clean
	rm -rf kernel/rust/target
