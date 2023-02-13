source_filename = "example/test.rj"

define void @test(i32* %0, i32* %1, i32* %2, i32* %3, i32* %4, i32* %5) {
  %7 = alloca i32

  %8 = alloca i32

  %9 = load i32, i32* %4
  store i32 %9, i32* %0
  %10 = load i32, i32* %5
  store i32 %10, i32* %1
  %11 = load i32, i32* %7
  store i32 %11, i32* %2
  %12 = load i32, i32* %8
  store i32 %12, i32* %3

  ret void
}
