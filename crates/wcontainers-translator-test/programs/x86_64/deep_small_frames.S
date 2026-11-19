// A million calls deep, each frame only the 8 bytes of its return address, then exits with the low byte of
// the sum of the depths.
.text
.globl _start
_start:
  mov $1000000, %edi
  xor %esi, %esi
  call down
  movzbl %sil, %edi
  mov $60, %eax
  syscall
down:
  test %rdi, %rdi
  jz 1f
  add %rdi, %rsi
  dec %rdi
  call down
1:
  ret
