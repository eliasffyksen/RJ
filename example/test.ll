; ModuleID = '<stdin>'
source_filename = "example/test.rj"

; Function Attrs: nofree norecurse nounwind writeonly
define void @test(i32* nocapture %0, i32* nocapture %1, i32* nocapture readnone %2, i32* nocapture readnone %3, i32 %4, i32 %5) local_unnamed_addr #0 {
  store i32 %4, i32* %0, align 4
  store i32 %5, i32* %1, align 4
  ret void
}

attributes #0 = { nofree norecurse nounwind writeonly }