// Pippin C glue — minimal libc stubs for the freestanding environment.
#include <pippin/klibc.h>

void* memset(void* dst, int value, size_t n) {
    unsigned char* p = (unsigned char*)dst;
    while (n--) {
        *p++ = (unsigned char)value;
    }
    return dst;
}

void* memcpy(void* dst, const void* src, size_t n) {
    unsigned char*       d = (unsigned char*)dst;
    const unsigned char* s = (const unsigned char*)src;
    while (n--) {
        *d++ = *s++;
    }
    return dst;
}

void* memmove(void* dst, const void* src, size_t n) {
    unsigned char*       d = (unsigned char*)dst;
    const unsigned char* s = (const unsigned char*)src;
    if (d < s) {
        while (n--) {
            *d++ = *s++;
        }
    } else if (d > s) {
        d += n;
        s += n;
        while (n--) {
            *--d = *--s;
        }
    }
    return dst;
}

int memcmp(const void* a, const void* b, size_t n) {
    const unsigned char* p = (const unsigned char*)a;
    const unsigned char* q = (const unsigned char*)b;
    while (n--) {
        if (*p != *q) {
            return (int)*p - (int)*q;
        }
        ++p;
        ++q;
    }
    return 0;
}

size_t strlen(const char* s) {
    const char* p = s;
    while (*p != '\0') {
        ++p;
    }
    return (size_t)(p - s);
}