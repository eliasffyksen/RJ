; Program entry point. Calls the RJ `main` function and exits with the value
; it returns, using the Linux x86-64 exit_group syscall directly so programs
; don't need libc or a dynamic linker.

declare void @rj_main(ptr)

define void @_start() noreturn alignstack(16) {
  %code = alloca i32

  ; Default to 1 so a `main` that never sets its return value fails.
  store i32 1, ptr %code

  call void @rj_main(ptr %code)
  %value = load i32, ptr %code
  %status = sext i32 %value to i64
  call void asm sideeffect "syscall", "{rax},{rdi},~{rcx},~{r11},~{memory}"(i64 231, i64 %status)
  unreachable
}
