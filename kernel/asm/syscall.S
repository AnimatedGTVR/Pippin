/* SYSCALL/SYSRET entry and a tiny relocatable ring-3 smoke program. */
.section .text
.code64
.extern pippin_syscall_kernel_rsp
.extern pippin_syscall_dispatch
.extern pippin_current_fx_area
.global pippin_syscall_entry
.type pippin_syscall_entry, @function
pippin_syscall_entry:
    /* IA32_FMASK has cleared IF; switch from user to this task's kernel stack. */
    movq %rsp, pippin_syscall_user_rsp(%rip)
    movq pippin_syscall_kernel_rsp(%rip), %rsp
    pushq %rcx
    pushq %r11
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    subq $8, %rsp
    /* C ABI: dispatch(number, arg0, arg1, arg2). */
    movq %rdx, %rcx
    movq %rsi, %rdx
    movq %rdi, %rsi
    movq %rax, %rdi
    movq pippin_current_fx_area(%rip), %rax
    fxsave64 (%rax)
    call pippin_syscall_dispatch
    movq pippin_current_fx_area(%rip), %rcx
    fxrstor64 (%rcx)
    addq $8, %rsp
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    popq %r11
    popq %rcx
    movq pippin_syscall_user_rsp(%rip), %rsp
    sysretq
.size pippin_syscall_entry, . - pippin_syscall_entry

.section .bss
.p2align 3
pippin_syscall_user_rsp:
    .quad 0

.section .note.GNU-stack,"",@progbits