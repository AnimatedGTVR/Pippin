// PCI configuration-space discovery and the first Driver Manager lifecycle.
#include <pippin/drivers.hh>
#include <stddef.h>

namespace {
inline void out32(uint16_t port, uint32_t value) {
    asm volatile("outl %0, %1" : : "a"(value), "Nd"(port));
}
inline uint32_t in32(uint16_t port) {
    uint32_t value;
    asm volatile("inl %1, %0" : "=a"(value) : "Nd"(port));
    return value;
}
uint32_t config(uint8_t bus, uint8_t device, uint8_t function, uint8_t offset) {
    out32(0xcf8, 0x80000000u | (uint32_t(bus) << 16) |
                     (uint32_t(device) << 11) | (uint32_t(function) << 8) |
                     (offset & 0xfc));
    return in32(0xcfc);
}
void write_config(uint8_t bus, uint8_t device, uint8_t function,
                  uint8_t offset, uint32_t value) {
    out32(0xcf8, 0x80000000u | (uint32_t(bus) << 16) |
                     (uint32_t(device) << 11) | (uint32_t(function) << 8) |
                     (offset & 0xfc));
    out32(0xcfc, value);
}

struct PciNode {
    uint16_t vendor, product;
    uint8_t bus, device, function, class_code, subclass, prog_if;
    int16_t parent;
};
constexpr size_t kMaxNodes = 128;
PciNode nodes[kMaxNodes];
size_t node_count = 0;
bool scanned = false;

class PciDriver final : public pippin::drv::Driver {
public:
    const char* name() const override { return "pci"; }
    bool probe() override { return config(0, 0, 0, 0) != 0xffffffffu; }
    int init() override {
        if (scanned) return 0;
        scanned = true;
        for (unsigned bus = 0; bus < 256; ++bus) {
            for (unsigned dev = 0; dev < 32; ++dev) {
                const uint32_t first = config(bus, dev, 0, 0);
                if ((first & 0xffffu) == 0xffffu) continue;
                const bool multi = (config(bus, dev, 0, 0x0c) & 0x00800000u) != 0;
                for (unsigned fn = 0; fn < (multi ? 8u : 1u); ++fn) {
                    const uint32_t id = fn ? config(bus, dev, fn, 0) : first;
                    if ((id & 0xffffu) == 0xffffu) continue;
                    const uint32_t kind = config(bus, dev, fn, 8);
                    if (node_count < kMaxNodes) {
                        nodes[node_count++] = {
                            uint16_t(id), uint16_t(id >> 16), uint8_t(bus),
                            uint8_t(dev), uint8_t(fn), uint8_t(kind >> 24),
                            uint8_t(kind >> 16), uint8_t(kind >> 8), -1,
                        };
                    }
                }
            }
        }
        // Attach devices behind PCI-to-PCI bridges to their nearest bridge.
        for (size_t i = 0; i < node_count; ++i) {
            if (nodes[i].bus == 0) continue;
            uint8_t best_secondary = 0;
            for (size_t j = 0; j < node_count; ++j) {
                const auto& bridge = nodes[j];
                if (bridge.class_code != 6 || bridge.subclass != 4) continue;
                const uint32_t buses = config(bridge.bus, bridge.device,
                                              bridge.function, 0x18);
                const uint8_t secondary = buses >> 8;
                const uint8_t subordinate = buses >> 16;
                if (secondary && secondary <= nodes[i].bus &&
                    nodes[i].bus <= subordinate && secondary >= best_secondary) {
                    nodes[i].parent = static_cast<int16_t>(j);
                    best_secondary = secondary;
                }
            }
        }
        return 0;
    }
};

PciDriver pci_driver;
pippin::drv::Driver* drivers[] = {&pci_driver};
unsigned active_drivers = 0;
}

extern "C" unsigned pippin_driver_count() { return active_drivers; }
extern "C" unsigned pippin_driver_probe_all() {
    if (active_drivers) return active_drivers;
    for (auto* driver : drivers) {
        if (driver->probe() && driver->init() == 0) ++active_drivers;
    }
    return active_drivers;
}
extern "C" unsigned pippin_pci_device_count() { return node_count; }
extern "C" int pippin_pci_parent(unsigned index) {
    return index < node_count ? nodes[index].parent : -2;
}
extern "C" bool pippin_pci_device(unsigned index, uint16_t* vendor,
                                     uint16_t* product, uint8_t* class_code,
                                     uint8_t* subclass) {
    if (index >= node_count) return false;
    const auto& node = nodes[index];
    *vendor = node.vendor;
    *product = node.product;
    *class_code = node.class_code;
    *subclass = node.subclass;
    return true;
}
extern "C" uint64_t pippin_ahci_bar() {
    for (size_t i = 0; i < node_count; ++i) {
        const auto& node = nodes[i];
        if (node.class_code != 1 || node.subclass != 6 || node.prog_if != 1) continue;
        const uint32_t bar = config(node.bus, node.device, node.function, 0x24);
        if ((bar & 1) || (bar & ~0xfu) == 0) continue;
        const uint32_t command = config(node.bus, node.device, node.function, 4);
        write_config(node.bus, node.device, node.function, 4, command | 6u);
        return bar & ~0xfu;
    }
    return 0;
}

// QEMU standard VGA exposes its 16 MiB linear framebuffer in PCI BAR0.
extern "C" uint64_t pippin_qemu_vga_bar() {
    for (size_t i = 0; i < node_count; ++i) {
        const auto& node = nodes[i];
        if (node.vendor != 0x1234 || node.product != 0x1111) continue;
        const uint32_t bar = config(node.bus, node.device, node.function, 0x10);
        if ((bar & 1) || (bar & 6) || (bar & ~0xfu) == 0) return 0;
        const uint32_t command = config(node.bus, node.device, node.function, 4);
        write_config(node.bus, node.device, node.function, 4, command | 2u);
        return bar & ~0xfu;
    }
    return 0;
}
