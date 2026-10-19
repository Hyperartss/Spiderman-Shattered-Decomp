// Phase 4 probe: what does Ghidra know about the XEX and the boot path?
//
// Answers three questions the runtime and codegen could not:
//   1. What lives at the two addresses the boot trap referenced?
//   2. What does the XEX module/section structure actually contain?
//   3. Who calls the XexLoadImage / XexGetProcedureAddress wrappers?
//
// Prints to stdout only; writes nothing back to the program.

import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.*;
import ghidra.program.model.address.*;

public class ProbeBootPath extends GhidraScript {

    private void describe(Address a, String label) {
        println("  " + label + " @ " + a);
        Function f = getFunctionAt(a);
        if (f == null) {
            f = getFunctionContaining(a);
            if (f != null) {
                println("      (inside " + f.getName() + " @ " + f.getEntryPoint() + ")");
            } else {
                println("      no function");
            }
        } else {
            println("      name    : " + f.getName());
            println("      signature: " + f.getSignature().getPrototypeString());
            println("      callers : " + f.getCallingFunctions(monitor).size());
            for (Function c : f.getCallingFunctions(monitor)) {
                println("        <- " + c.getName() + " @ " + c.getEntryPoint());
            }
        }
    }

    @Override
    public void run() throws Exception {
        println("=== PROGRAM ===");
        println("  name    : " + currentProgram.getName());
        println("  language: " + currentProgram.getLanguageID());
        println("  image base: " + currentProgram.getImageBase());
        println("  min/max addr: " + currentProgram.getMinAddress() + " - " + currentProgram.getMaxAddress());

        FunctionManager fm = currentProgram.getFunctionManager();
        println("  functions discovered: " + fm.getFunctionCount());

        println();
        println("=== MEMORY BLOCKS (section layout) ===");
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            println(String.format("  %-12s %s - %s  size=0x%x  init=%s r=%b w=%b x=%b",
                b.getName(), b.getStart(), b.getEnd(),
                b.getSize(), b.isInitialized(), b.isRead(), b.isWrite(), b.isExecute()));
        }

        println();
        println("=== BOOT PATH ADDRESSES ===");
        long[] targets = {
            0x8218dd48L, 0x8218df20L, 0x82200aa8L, 0x821ffba8L, 0x8218db18L,
            0x82220370L, 0x82220330L, 0x880dce78L, 0x880d0040L, 0x88678bb8L
        };
        for (long t : targets) {
            Address a = toAddr(t);
            String inBlock = "(unmapped)";
            for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
                if (b.getStart().compareTo(a) <= 0 && a.compareTo(b.getEnd()) <= 0) {
                    inBlock = "in block " + b.getName();
                    break;
                }
            }
            println("  0x" + Long.toHexString(t) + "  " + inBlock);
            describe(a, "0x" + Long.toHexString(t));
        }

        println();
        println("=== IMPORTED / NAMED KERNEL SYMBOLS PRESENT ===");
        SymbolTable st = currentProgram.getSymbolTable();
        int shown = 0;
        SymbolIterator it = st.getSymbolIterator("Xex*", true);
        while (it.hasNext() && shown < 25) {
            Symbol s = it.next();
            println("  " + s.getName() + " @ " + s.getAddress());
            shown++;
        }
        if (shown == 0) {
            println("  (none named Xex* -- import renamer may not have run)");
        }
    }
}