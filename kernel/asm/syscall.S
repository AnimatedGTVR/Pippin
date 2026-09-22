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

/* This page is copied to low user memory before entering ring 3. */
.section .rodata.user_demo, "a", @progbits
.p2align 4
.global pippin_user_demo_start
.global pippin_user_demo_end
pippin_user_demo_start:
    movq %rdi, %r12              /* port ID from synthetic interrupt frame */
    movq $2, %rax                /* MMAP one user page */
    movq $0x600000, %rdi
    syscall
    cmpq $0x600000, %rax
    jne 1f
    movb $0x5a, (%rax)
    movq $3, %rax                /* IPC_SEND(port, kind, value) */
    movq %r12, %rdi
    movq $9, %rsi
    movq $0xC0DE, %rdx
    syscall
    movq $1, %rax                /* LOG(pointer, length) */
    leaq .Luser_msg(%rip), %rdi
    movq $(.Luser_msg_end - .Luser_msg), %rsi
    syscall
    movq $0, %rax                /* EXIT */
    syscall
1:  pause
    jmp 1b
.Luser_msg:
    .ascii "  syscall:          ring-3 log and IPC worked\n"
.Luser_msg_end:
pippin_user_demo_end:

.section .note.GNU-stack,"",@progbits
