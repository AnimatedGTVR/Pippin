/* Pippin boot.S — Multiboot entry point, long-mode bring-up.            */
/*                                                                       */
/* The bootloader (QEMU / GRUB, Multiboot v1) drops us here in 32-bit    */
/* protected mode with paging disabled (or unknown). We:                 */
/*   1. save the multiboot magic + info pointer,                         */
/*   2. zero the low boot BSS (page tables + stack + scratch),           */
/*   3. build a flat identity map of the low 1 GiB with 2 MiB pages AND  */
/*      a higher-half alias of the same 1 GiB at KERNEL_VMA_BASE,        */
/*   4. enable PAE + EFER.LME + paging and load a 64-bit GDT,            */
/*   5. far-jump into 64-bit mode (still low), then hop to the           */
/*      higher-half entry and call the C++ entry point.                  */
/*                                                                       */
/* Everything below runs from the LOW .boot.* sections linked at        */
/* physical KERNEL_LOAD_BASE. Once paging is on, both the low identity   */
/* window and the high alias are live, so the hop is a plain jump.       */
/* Symbols __boot_bss_start/__boot_bss_end come from kernel/linker.ld.   */
/* Frames used by the low page tables are never returned.                */

/* --- Multiboot v1 header (must live in the first 8 KiB of the image) - */
/*                                                                       */
/* We set MULTIBOOT_HEADER_HAS_ADDR and list the load/bss/entry addresses */
/* explicitly. QEMU <= 8.x refuses 64-bit *ELF* kernels (it only lets    */
/* 32-bit ELFs through load_elf), but with the ADDR fields it byte-copies */
/* the image straight to RAM and never inspects the ELF, so a 64-bit     */
/* kernel boots fine. GRUB handles both paths.                           */
/*                                                                       */
/* The link script keeps .multiboot in the low boot segment, so the      */
/* address fields are the *physical* (load) addresses here.              */
.set ALIGN,    1 << 0
.set MEMINFO,  1 << 1
.set ADDR,     1 << 16                 /* address fields follow checksum   */
.set FLAGS,    ALIGN | MEMINFO | ADDR
.set MAGIC,    0x1BADB002
.set CHECKSUM, -(MAGIC + FLAGS)

.section .multiboot, "a"
.align 4
.long MAGIC
.long FLAGS
.long CHECKSUM
.long 0x100000        /* header_addr:  physical address of this header      */
.long 0x100000        /* load_addr:    physical address of the image start  */
.long __data_end_phys /* load_end_addr: end of file-backed data (physical)  */
.long __bss_end_phys  /* bss_end_addr:  end of BSS (physical)              */
.long _start          /* entry_addr:    physical entry point                */

/* --- Boot scratch: page tables + early stack (cleared by _start) ------ */
/* Alloc-only (no W): keeps ld from bundling it into the high RW segment,
   whose file offsets would no longer track the physical layout QEMU's
   Multiboot copy expects. Runtime writability comes from our page tables,
   not from ELF section flags. QEMU + boot.S both zero this memory. */
.section .boot.bss, "a", @progbits
.align 4096
boot_pml4:
  .skip 4096
boot_pdpt:
  .skip 4096
boot_pd0:
  .skip 4096          /* maps the low 1 GiB with 2 MiB pages            */
boot_stack_bottom:
  .skip 32768          /* 32 KiB early kernel stack                      */
.global boot_stack_top
boot_stack_top:

/* --- Non-zeroed early data (survives the BSS zeroing pass) ------------ */
.section .boot.data, "aw", @progbits
.balign 4
boot_magic:   .skip 4   /* saved %eax from the bootloader                 */
boot_mbinfo:  .skip 8   /* saved %ebx multiboot info address              */

/* --- 32-bit entry (low boot segment) ----------------------------------- */
.section .boot.text, "ax", @progbits
.code32
.global _start
.type _start, @function
_start:
    cli
    mov  $boot_stack_top, %esp

    mov  %eax, boot_magic       /* save multiboot magic                   */
    mov  %ebx, boot_mbinfo      /* save multiboot info pointer            */

    /* Zero the low boot BSS: page tables, stack, scratch (the loader
       zeroes the *high* BSS via bss_end_addr, not this window).         */
    mov  $__boot_bss_start, %edi
    mov  $__boot_bss_end, %ecx
    sub  %edi, %ecx
    xor  %eax, %eax
    rep stosb

    /* Verify that the bootloader really is Multiboot.                    */
    cmpl $0x2BADB002, boot_magic
    jne  .Lhang

    /* CPUID: can we toggle the ID bit?                                   */
    pushfl
    pop  %eax
    mov  %eax, %ecx
    xor  $0x200000, %eax        /* bit 21 = CPUID ID flag                 */
    push %eax
    popfl
    pushfl
    pop  %eax
    cmp  %ecx, %eax
    je   .Lhang

    /* Long mode available? leaf 0x80000001, bit 29 (LM).                 */
    mov  $0x80000000, %eax
    cpuid
    cmp  $0x80000001, %eax
    jb   .Lhang
    mov  $0x80000001, %eax
    cpuid
    test $0x20000000, %edx      /* bit 29 = long mode                    */
    jz   .Lhang

    /* Build boot_pd0: 512 huge-page entries, present|writable.           */
    lea  boot_pd0, %edi
    mov  $512, %ecx
    mov  $0x83, %eax            /* present | writable | 2 MiB page        */
1:  mov  %eax, (%edi)
    add  $8, %edi
    add  $0x200000, %eax        /* next 2 MiB                             */
    loop 1b

    /* Point PDPT[0] at boot_pd0 and PML4[0] at boot_pdpt (identity).     */
    mov  $boot_pd0, %eax
    or   $0x3, %eax
    mov  %eax, boot_pdpt

    mov  $boot_pdpt, %eax
    or   $0x3, %eax
    mov  %eax, boot_pml4

    /* PML4[256] -> the SAME PDPT/PD: the low 1 GiB is aliased at
       KERNEL_VMA_BASE (0xFFFF800000000000 == sign-extended bit 47, so it
       lives in PML4 slot 256) and the higher-half hop is already mapped
       the moment paging comes on.                                       */
    mov  %eax, boot_pml4 + (256 * 8)

    /* Load paging. cr3 <- PML4, PAE on, EFER.LME on, PG+WP on.           */
    mov  $boot_pml4, %eax
    mov  %eax, %cr3

    mov  %cr4, %eax
    or   $0x20, %eax               /* PAE                                   */
    or   $0x200, %eax              /* OSFXSR: SSE (FXSAVE) enabled; LLVM    */
    or   $0x400, %eax              /* OSXMMEXCPT: XMM exceptions enabled    */
    mov  %eax, %cr4

    mov  $0xC0000080, %ecx      /* EFER MSR                              */
    rdmsr
    or   $0x100, %eax           /* LME                                   */
    wrmsr

    mov  %cr0, %eax
    or   $0x80010000, %eax      /* PG | WP                               */
    mov  %eax, %cr0

    lgdt gdtr
    ljmp $0x08, $.Llow64        /* jump to 64-bit code segment (low)      */

.global .Llow64
.code64
.Llow64:
    /* Running at a LOW address; the whole low 1 GiB plus its alias at
       KERNEL_VMA_BASE are mapped. Hop into the higher-half kernel.       */
    movabs $__kernel_high_entry, %rax
    jmp *%rax

.Lhang:
    cli
1:  hlt
    jmp 1b
.size _start, . - _start

/* --- 64-bit GDT (kept low; identity-mapped for the lifetime of the
       early window, so the loaded gdtr stays valid) ---------------------- */
.section .boot.rodata, "a", @progbits
.align 8
gdt64:
    .quad 0x0000000000000000         /* null                             */
    .quad 0x00AF9A000000FFFF         /* ring 0 code, L-bit set           */
    .quad 0x00CF92000000FFFF         /* ring 0 data                      */
gdtr:
    .word (gdtr - gdt64) - 1
    .quad gdt64

/* --- Higher-half kernel entry (linked at KERNEL_VMA_BASE) -------------- */
.section .text
.code64
.global __kernel_high_entry
.type __kernel_high_entry, @function
__kernel_high_entry:
    xor  %eax, %eax
    mov  $0x10, %ax
    mov  %ax, %ds
    mov  %ax, %es
    mov  %ax, %fs
    mov  %ax, %gs
    mov  %ax, %ss

    movabsq $boot_mbinfo, %rdi  /* low scratch address (64-bit immediate)   */
    mov  (%rdi), %edi           /* multiboot info pointer -> first arg       */
    and  $-16, %rsp              /* align stack for the ABI               */
    call kernel_entry

.Lhang_high:
    cli
1:  hlt
    jmp 1b
.size __kernel_high_entry, . - __kernel_high_entry

/* Mark the stack non-executable (linker warns otherwise).                */
.section .note.GNU-stack,"",@progbits
