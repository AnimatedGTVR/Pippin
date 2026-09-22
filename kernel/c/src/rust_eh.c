// Rust unwinding stubs for the freestanding kernel.
//
// The sysroot `alloc` crate is precompiled for the host, whose personality
// still references rust_eh_personality and _Unwind_Resume. The kernel builds
// with panic=abort, so these paths are never actually taken; the stubs exist
// only to satisfy the link. Should a panic ever try to unwind, we hang loudly
// rather than corrupt the machine.

// Returns _URC_NO_REASON (0): no foreign exception was received; with
// panic=abort nothing ever unwinds into a landing pad.
int rust_eh_personality(int version,
                        int actions,
                        unsigned long long exception_class,
                        void* exception_object)
{
    (void)version; (void)actions;
    (void)exception_class; (void)exception_object;
    return 0;
}

__attribute__((noreturn))
void _Unwind_Resume(void* exception_object)
{
    (void)exception_object;
    for (;;) {
        __asm__ __volatile__("hlt");
    }
}