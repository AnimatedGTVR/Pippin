// Pippin C glue — libc-style stubs required by the freestanding C++/Rust
// toolchains. These are the only functions the kernel implements in C; the
// module exists as the ABI-shim layer and as a place for legacy C driver
// sources until they are ported to C++.
#pragma once

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

void* memset(void* dst, int value, size_t n);
void* memcpy(void* dst, const void* src, size_t n);
void* memmove(void* dst, const void* src, size_t n);
int   memcmp(const void* a, const void* b, size_t n);
size_t strlen(const char* s);

#ifdef __cplusplus
}
#endif