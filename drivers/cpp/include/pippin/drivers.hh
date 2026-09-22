// Pippin Driver Layer (C++) — public driver ABI.
//
// Drivers register with the Driver Manager through this interface. The
// Manager owns the device tree and the hardware probe/init lifecycle; the
// skeleton only proves the linkage across the language boundary.
#pragma once

#include <stdint.h>

namespace pippin {
namespace drv {

// Bus/device classes (subset; grows with the driver tree).
enum class DeviceClass : uint32_t {
    kUnknown = 0,
    kPci = 1,
    kSerial = 2,
    kDisplay = 3,
    kInput = 4,
    kStorage = 5,
};

// Every Pippin driver derives from Driver and implements the lifecycle
// hooks. Manager code calls these through plain C thunks so the Rust core
// can drive them too (docs/architecture.md, § Driver model).
class Driver {
   public:
    ~Driver() = default;
    virtual const char* name() const = 0;
    virtual bool probe() { return false; }
    virtual int init() { return 0; }
};

// Driver Manager entry: returns the count of drivers currently registered.
// Implemented in C directly so the Rust core can query it via extern "C".
extern "C" unsigned pippin_driver_count(void);

}  // namespace drv
}  // namespace pippin
