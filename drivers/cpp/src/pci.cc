// Pippin PCI driver source — Milestone-3 milestone. For now it only fills
// the driver registry so the cross-language call path can be exercised.
#include <pippin/drivers.hh>

extern "C" unsigned pippin_driver_count(void) {
    return 0u;
}