/* A flat blob: code first, starting with _start, then read-only data, then data with its zeroed part included. */
ENTRY(_start)
SECTIONS {
  . = 0x400000;
  .text : { *(.text._start) *(.text .text.*) }
  . = ALIGN(16);
  .rodata : { *(.rodata .rodata.*) }
  . = ALIGN(16);
  .data : { *(.data .data.*) *(.bss .bss.*) *(COMMON) }
  /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) *(.ARM.attributes) }
}
