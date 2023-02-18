
declare void @exit(i32)
declare void @main(i32*)

define void @_start() {
  %1 = alloca i32

  ; Set default return to 1
  ; in order to fail if main does not set return code
  store i32 1, i32* %1

  call void @main(i32* %1)
  %2 = load i32, i32* %1
  call void @exit(i32 %2)
  unreachable
}
