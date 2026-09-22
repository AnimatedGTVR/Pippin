ENTRY(_start)

PHDRS
{
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}

SECTIONS
{
    /* Keep user applications well above Pippin's 1 GiB kernel direct map so
       user mappings never replace the kernel's physical identity window. */
    . = 0x0000000100000000;

    .text : ALIGN(0x1000)
    {
        *(.text .text.*)
        *(.rodata .rodata.*)
    } :text

    . = ALIGN(0x1000);
    .data :
    {
        *(.data .data.*)
    } :data

    .bss : ALIGN(16)
    {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    /DISCARD/ :
    {
        *(.comment)
        *(.eh_frame*)
        *(.note .note.*)
        *(.rela.*)
        *(.dynamic)
        *(.dynsym)
        *(.dynstr)
        *(.gnu*)
    }
}
