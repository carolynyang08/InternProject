; ModuleID = 'probe6.b0469fe5-cgu.0'
source_filename = "probe6.b0469fe5-cgu.0"
target datalayout = "e-m:e-p:32:32-Fi8-i64:64-v128:64:128-a:0:32-n32-S64"
target triple = "thumbv7em-none-unknown-eabihf"

@alloc_26948764b08a3a591a9d2a7abf3d71a6 = private unnamed_addr constant <{ [75 x i8] }> <{ [75 x i8] c"/rustc/90c541806f23a127002de5b4038be731ba1458ca/library/core/src/num/mod.rs" }>, align 1
@alloc_3f7bec02a5727a67e50c00ab33a6bceb = private unnamed_addr constant <{ ptr, [12 x i8] }> <{ ptr @alloc_26948764b08a3a591a9d2a7abf3d71a6, [12 x i8] c"K\00\00\00/\04\00\00\05\00\00\00" }>, align 4
@str.0 = internal constant [25 x i8] c"attempt to divide by zero"

; probe6::probe
; Function Attrs: nounwind
define dso_local void @_ZN6probe65probe17h60da7ab4d0d6a4f6E() unnamed_addr #0 {
start:
  %0 = call i1 @llvm.expect.i1(i1 false, i1 false)
  br i1 %0, label %panic.i, label %"_ZN4core3num21_$LT$impl$u20$u32$GT$10div_euclid17he1a6579d5ce532a1E.exit"

panic.i:                                          ; preds = %start
; call core::panicking::panic
  call void @_ZN4core9panicking5panic17hef92580aa583e801E(ptr align 1 @str.0, i32 25, ptr align 4 @alloc_3f7bec02a5727a67e50c00ab33a6bceb) #3
  unreachable

"_ZN4core3num21_$LT$impl$u20$u32$GT$10div_euclid17he1a6579d5ce532a1E.exit": ; preds = %start
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(none)
declare i1 @llvm.expect.i1(i1, i1) #1

; core::panicking::panic
; Function Attrs: cold noinline noreturn nounwind
declare dso_local void @_ZN4core9panicking5panic17hef92580aa583e801E(ptr align 1, i32, ptr align 4) unnamed_addr #2

attributes #0 = { nounwind "frame-pointer"="all" "target-cpu"="generic" "target-features"="+vfp4,-d32,-fp64" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(none) }
attributes #2 = { cold noinline noreturn nounwind "frame-pointer"="all" "target-cpu"="generic" "target-features"="+vfp4,-d32,-fp64" }
attributes #3 = { noreturn nounwind }
