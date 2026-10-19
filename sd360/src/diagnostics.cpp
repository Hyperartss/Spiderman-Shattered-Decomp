// Phase 4: stop the boot trap by short-circuiting XEX procedure lookup.
//
// FINDING (traced with temporary hooks on sub_82220370 / sub_82220330):
//
// The boot trap is an indirect call in sub_8218DD48 whose target is the
// return value of
//
//   sub_82220370() -> XexLoadImage(ordinal=9, ...)            -> handle
//   sub_82220330() -> XexGetProcedureAddress(handle, ordinal) -> proc addr
//
// Traced output:
//
//   XexLoadImage(ordinal=9)                             -> 0x30099000
//   XexGetProcedureAddress(handle=0x30099000, ordinal=1) -> 0x880D0040
//   XexGetProcedureAddress(handle=0x30099000, ordinal=2) -> 0x880DCE78
//
// 0x880DCE78 is what lands in CTR and gets called with bctrl, reaching
// rex::runtime::InvalidFunctionTrap. That address is a host-function
// trampoline minted by the SDK, and the indirect-call dispatcher does not
// resolve it.
//
// Why this file overrides the guest wrapper rather than the import:
//
// REX_EXPORT_STUB_RETURN(XexGetProcedureAddress, 0) does NOT work here.
// __imp__XexGetProcedureAddress is a strong (T) symbol in librexruntimed.so,
// so the linker binds the generated call straight to the SDK implementation
// and the registry-based export stub never sees it. Overriding that symbol
// would mean patching the SDK.
//
// sub_82220330 is different: it is recompiled game code, emitted as a weak
// alias to __imp__sub_82220330. Defining a strong symbol of the same name
// overrides it cleanly, with no SDK change. That is the documented override
// path (see the Function Overrides wiki page).
//
// Behaviour: report "procedure not present" by returning NULL. The guest
// already handles this -- sub_8218DD48 compares the result against zero and
// branches past the bctrl when it is NULL -- so boot continues instead of
// aborting.
//
// Replace with a real implementation once module ordinal 9 is understood.

#include <rex/hook.h>

#include <cstdio>

// XexGetProcedureAddress(handle, ordinal) -> procedure address.
// Returning 0 means the procedure is not present.
REX_HOOK_RAW(sub_82220330) {
  static uint32_t call_count = 0;
  std::fprintf(stderr, "[modstub] sub_82220330 handle=0x%08X ordinal=%u -> NULL\n",
               ctx.r3.u32, ctx.r4.u32);
  std::fflush(stderr);

  ctx.r3.s64 = 0;

  if (++call_count <= 0xFFFFFFFFu) {
    // Intentionally does not call __imp__sub_82220330: the SDK implementation
    // returns trampoline addresses that the dispatcher cannot resolve.
  }
}