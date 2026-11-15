/* A flat blob: code first, starting with _start, then read-only data, then data with its zeroed part included.
   A flat copy leaves out zeroed data at the end, so one byte with contents follows it. */
ENTRY(_start)
SECTIONS {
  . = 0x400000;
  .text : { *(.text._start) *(.text .text.*) }
  . = ALIGN(16);
  .rodata : { *(.rodata .rodata.*) }
  . = ALIGN(16);
  .data : { *(.data .data.*) *(.bss .bss.*) *(COMMON) }
  .end : { BYTE(0) }
  /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) *(.ARM.attributes) }
}
