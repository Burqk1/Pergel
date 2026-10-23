// Bridges the prebuilt Skia (compiled with Emscripten's JavaScript-based setjmp/longjmp) to a
// Rust toolchain that links with native WebAssembly exceptions, where those JS helpers do not
// exist. FreeType, libpng and libjpeg only longjmp on malformed data; the normal path just calls
// through, so the `invoke_*` trampolines call the target directly and a longjmp aborts instead.
//
// The signatures are every `invoke_*` that libskia.a imports (llvm-nm -A libskia.a | grep invoke_):
// the first letter is the return type, the rest the arguments; all are i32 here.

#include <stdint.h>
#include <stdlib.h>

#define FN(T, p) ((T)(uintptr_t)(p))

int invoke_ii(int f, int a0) { return FN(int (*)(int), f)(a0); }
int invoke_iii(int f, int a0, int a1) { return FN(int (*)(int, int), f)(a0, a1); }
int invoke_iiii(int f, int a0, int a1, int a2) { return FN(int (*)(int, int, int), f)(a0, a1, a2); }
int invoke_iiiii(int f, int a0, int a1, int a2, int a3) { return FN(int (*)(int, int, int, int), f)(a0, a1, a2, a3); }
int invoke_iiiiii(int f, int a0, int a1, int a2, int a3, int a4) { return FN(int (*)(int, int, int, int, int), f)(a0, a1, a2, a3, a4); }
int invoke_iiiiiii(int f, int a0, int a1, int a2, int a3, int a4, int a5) { return FN(int (*)(int, int, int, int, int, int), f)(a0, a1, a2, a3, a4, a5); }
int invoke_iiiiiiii(int f, int a0, int a1, int a2, int a3, int a4, int a5, int a6) { return FN(int (*)(int, int, int, int, int, int, int), f)(a0, a1, a2, a3, a4, a5, a6); }
int invoke_iiiiiiiiii(int f, int a0, int a1, int a2, int a3, int a4, int a5, int a6, int a7, int a8) { return FN(int (*)(int, int, int, int, int, int, int, int, int), f)(a0, a1, a2, a3, a4, a5, a6, a7, a8); }
void invoke_v(int f) { FN(void (*)(void), f)(); }
void invoke_vi(int f, int a0) { FN(void (*)(int), f)(a0); }
void invoke_vii(int f, int a0, int a1) { FN(void (*)(int, int), f)(a0, a1); }
void invoke_viii(int f, int a0, int a1, int a2) { FN(void (*)(int, int, int), f)(a0, a1, a2); }
void invoke_viiii(int f, int a0, int a1, int a2, int a3) { FN(void (*)(int, int, int, int), f)(a0, a1, a2, a3); }
void invoke_viiiii(int f, int a0, int a1, int a2, int a3, int a4) { FN(void (*)(int, int, int, int, int), f)(a0, a1, a2, a3, a4); }
void invoke_viiiiii(int f, int a0, int a1, int a2, int a3, int a4, int a5) { FN(void (*)(int, int, int, int, int, int), f)(a0, a1, a2, a3, a4, a5); }
void invoke_viiiiiiiii(int f, int a0, int a1, int a2, int a3, int a4, int a5, int a6, int a7, int a8) { FN(void (*)(int, int, int, int, int, int, int, int, int), f)(a0, a1, a2, a3, a4, a5, a6, a7, a8); }

void emscripten_longjmp(uintptr_t env, int value) {
    (void)env;
    (void)value;
    abort();
}
