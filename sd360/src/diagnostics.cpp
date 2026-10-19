// Phase 4: trace the XEX module load + procedure lookup, then let it run.
//
// CORRECTION to earlier notes in docs/runtime-status.md: XexLoadImage's first
// argument is a module NAME string, not an ordinal. The SDK signature is
//
//   u32 XexLoadImage_entry(mapped_string module_name, u32 module_flags,
//                          u32 min_version, mapped_u32 hmodule_ptr)
//
// so `li r4,9` in the generated code is module_flags, and r3 holds the name.
// Earlier reading of this as "ordinal 9" was wrong.
//
// Chain being traced (Ghidra decompile of Function_8218DD48):
//   XexLoadImage(name)                 -> hmodule
//   XexGetProcedureAddress(hmodule, 1) -> stored unguarded, used later
//   XexGetProcedureAddress(hmodule, 2) -> null-checked, called immediately
//
// These wrappers call through to the SDK implementation; the NULL override
// from earlier was harmful and has been removed.

#include <rex/hook.h>

#include <cstdio>

REX_EXTERN(__imp__sub_82220370);
REX_EXTERN(__imp__sub_82220330);

// Wrapper for XexLoadImage. r3 is a guest pointer to the module name string.
REX_HOOK_RAW(sub_82220370) {
  const uint32_t name_addr = ctx.r3.u32;
  const uint32_t flags = ctx.r4.u32;
  const uint32_t minver = ctx.r5.u32;

  // Guest addresses are offsets into `base` (the virtual memory base), so the
  // module name string can be read straight out of guest memory.
  const char* name_str = reinterpret_cast<const char*>(base + name_addr);
  std::fprintf(stderr, "[xex] XexLoadImage(name=\"%.64s\" @0x%08X, flags=%u, minver=%u)\n",
               name_str, name_addr, flags, minver);
  std::fflush(stderr);

  __imp__sub_82220370(ctx, base);

  std::fprintf(stderr, "[xex] XexLoadImage -> hmodule=0x%08X\n", ctx.r3.u32);
  std::fflush(stderr);
}

// Wrapper for XexGetProcedureAddress.
REX_HOOK_RAW(sub_82220330) {
  const uint32_t hmodule = ctx.r3.u32;
  const uint32_t ordinal = ctx.r4.u32;

  __imp__sub_82220330(ctx, base);

  std::fprintf(stderr, "[xex] XexGetProcedureAddress(hmodule=0x%08X, ordinal=%u) -> 0x%08X\n",
               hmodule, ordinal, ctx.r3.u32);
  std::fflush(stderr);
}