/* A flat blob: code first, starting with _start (the C library's, from crt1.o or rcrt1.o, when there is one),
   then read-only data, then data with its zeroed part included. A flat copy leaves out zeroed data at the
   end, so one byte with contents follows it. Unwind tables are kept as read-only data, with their index in
   a section of its own, which the unwinder finds through its program header. A position independent
   program's dynamic sections are kept for its own relocation at start, which finds them through the
   program headers the runners place after the blob. Thread local data is placed and found through its
   program header too; the image has no header of the program headers themselves, so the C library finds it
   only when the program is not moved. */
ENTRY(_start)
SECTIONS {
  . = 0x400000;
  /* libc++'s operators that a program may replace are in a section of their own, found by its bounds. */
  .text : {
    *crt1.o(.text .text.*) *(.text._start) *(.text .text.*) *(.init) *(.fini)
    PROVIDE_HIDDEN(__start___lcxx_override = .);
    *(__lcxx_override)
    PROVIDE_HIDDEN(__stop___lcxx_override = .);
  }
  . = ALIGN(16);
  .rodata : { *(.rodata .rodata.*) *(.gcc_except_table*) }
  .eh_frame_hdr : { *(.eh_frame_hdr) }
  .eh_frame : { KEEP(*(.eh_frame)) }
  .dynsym : { *(.dynsym) }
  .dynstr : { *(.dynstr) }
  .hash : { *(.hash) }
  .gnu.hash : { *(.gnu.hash) }
  .rela.dyn : { *(.rela.*) }
  .relr.dyn : { *(.relr.*) }
  /* What the program writes starts on a page of its own, as a linker's writable segment does, so no page
     holds both code and data the program writes. */
  . = ALIGN(CONSTANT(COMMONPAGESIZE));
  .dynamic : { *(.dynamic) }
  .tdata : { *(.tdata .tdata.*) }
  .tbss : { *(.tbss .tbss.*) }
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
