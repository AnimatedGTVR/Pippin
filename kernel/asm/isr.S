/* isr.S — normalized IDT stubs and the shared interrupt frame.            */
/*                                                                         */
/* A clean interrupt stack (low -> high addresses):                        */
/*   [GPRs (15 x u64)] [vector] [err] [rip] [cs] [rflags] [rsp] [ss]      */
/* The Rust dispatcher `pippin_int_dispatch` walks that frame; see         */
/* kernel/rust/src/idt.rs.                                                 */

.section .text
.extern pippin_int_dispatch
.extern pippin_current_fx_area

/* Stub for vectors with no CPU error code: pad one in. */
.macro ISR_STUB vector
.p2align 4
isr_\vector:
    pushq $0
    pushq $\vector
    jmp isr_common
.endm

/* Stub for vectors that carry a CPU error code. */
.macro ISR_STUB_ERR vector
.p2align 4
isr_\vector:
    pushq $\vector
    jmp isr_common
.endm

/* Generate stubs 0..255; the vectors that push a CPU error code take the
   ERR variant. Gas leaves `\vector` as the literal numeral from .irp.    */
.irp vector,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127,128,129,130,131,132,133,134,135,136,137,138,139,140,141,142,143,144,145,146,147,148,149,150,151,152,153,154,155,156,157,158,159,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,201,202,203,204,205,206,207,208,209,210,211,212,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227,228,229,230,231,232,233,234,235,236,237,238,239,240,241,242,243,244,245,246,247,248,249,250,251,252,253,254,255
.if \vector == 8 | \vector == 10 | \vector == 11 | \vector == 12 | \vector == 13 | \vector == 14 | \vector == 17 | \vector == 21
ISR_STUB_ERR \vector
.else
ISR_STUB \vector
.endif
.endr

/* --- Shared prologue/epilogue ----------------------------------------- */
isr_common:
    pushq %rax
    pushq %rbx
    pushq %rcx
    pushq %rdx
    pushq %rsi
    pushq %rdi
    pushq %rbp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    cld
    movq pippin_current_fx_area(%rip), %rax
    testq %rax, %rax
    jz 1f
    fxsave64 (%rax)
1:
    movq %rsp, %rdi
    andq $-16, %rsp
    call pippin_int_dispatch
    movq %rax, %rsp
    movq pippin_current_fx_area(%rip), %rax
    testq %rax, %rax
    jz 2f
    fxrstor64 (%rax)
2:
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rbp
    popq %rdi
    popq %rsi
    popq %rdx
    popq %rcx
    popq %rbx
    popq %rax
    addq $16, %rsp                    /* drop [vector][err]                */
    iretq

/* --- Address table for the Rust IDT builder (index = vector) ---------- */
.section .rodata
.p2align 3
.global isr_stub_table
isr_stub_table:
.irp vector,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127,128,129,130,131,132,133,134,135,136,137,138,139,140,141,142,143,144,145,146,147,148,149,150,151,152,153,154,155,156,157,158,159,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,201,202,203,204,205,206,207,208,209,210,211,212,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227,228,229,230,231,232,233,234,235,236,237,238,239,240,241,242,243,244,245,246,247,248,249,250,251,252,253,254,255
    .quad isr_\vector
.endr

.section .note.GNU-stack,"",@progbits
