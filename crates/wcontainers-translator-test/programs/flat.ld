/* A flat blob: code first, starting with _start (the C library's, from crt1.o or rcrt1.o, when there is one),
   then read-only data, then data with its zeroed part included. A flat copy leaves out zeroed data at the
   end, so one byte with contents follows it. Unwind tables are kept as read-only data (gcc's start files
   refer to them). A position independent program's dynamic sections are kept for its own relocation at
   start, which finds them through the program headers the runners place after the blob. The image holds
   no program headers itself, so thread local data is not supported. */
ENTRY(_start)
SECTIONS {
  . = 0x400000;
  .text : { *crt1.o(.text .text.*) *(.text._start) *(.text .text.*) *(.init) *(.fini) }
  . = ALIGN(16);
  .rodata : { *(.rodata .rodata.*) *(.eh_frame*) *(.gcc_except_table*) }
  .dynsym : { *(.dynsym) }
  .dynstr : { *(.dynstr) }
  .hash : { *(.hash) }
  .gnu.hash : { *(.gnu.hash) }
  .rela.dyn : { *(.rela.*) }
  .relr.dyn : { *(.relr.*) }
  . = ALIGN(16);
  .dynamic : { *(.dynamic) }
  .data : {
    PROVIDE_HIDDEN(__init_array_start = .);
    KEEP(*(SORT_BY_INIT_PRIORITY(.init_array.*) .init_array))
    PROVIDE_HIDDEN(__init_array_end = .);
    PROVIDE_HIDDEN(__fini_array_start = .);
    KEEP(*(SORT_BY_INIT_PRIORITY(.fini_array.*) .fini_array))
    PROVIDE_HIDDEN(__fini_array_end = .);
    *(.got .got.*) *(.data .data.*) *(.bss .bss.*) *(COMMON)
  }
  .end : { BYTE(0) }
  PROVIDE_HIDDEN(__blob_end = .);
  /DISCARD/ : { *(.comment) *(.note*) *(.ARM.attributes) *(.interp) }
}
