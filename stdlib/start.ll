
declare void @exit(i32)
declare void @main(i32*)

define void @_start() {
  %1 = alloca i32
  call void @main(i32* %1)
  %2 = load i32, i32* %1
  call void @exit(i32 %2)
  unreachable
}
