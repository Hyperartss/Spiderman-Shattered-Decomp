// Why did codegen not discover the function the runtime traps on?
//
// The runtime traps on guest address 0x88212EE0 inside GameLogic.dll, whose
// module base is 0x880D0000. Offset into the module = 0x142EE0. Codegen
// registered 21046 functions for this module, so this one was missed.
//
// Prints, for the faulting address and its neighbourhood:
//   - whether any function starts at or contains the address
//   - the raw bytes and how they disassemble
//   - whether the preceding bytes look like a valid function tail (which is
//     the signature of a function that ran past its discovered boundary)
//   - nearby string references that may explain what the code does

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.listing.*;
import ghidra.app.decompiler.*;

public class ProbeMissedFunction extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] a0 = getScriptArgs();
        long moduleBase = (a0.length > 0) ? Long.decode(a0[0]) : 0x880D0000L;
        long fault = (a0.length > 1) ? Long.decode(a0[1]) : 0x88212EE0L;
        long offset = fault - moduleBase;

        println("=== TARGET ===");
        println("  module base : " + toAddr(moduleBase));
        println("  fault addr  : " + toAddr(fault));
        println("  module off   : 0x" + Long.toHexString(offset));

        FunctionManager fm = currentProgram.getFunctionManager();
        Address a = toAddr(fault);

        println();
        println("=== FUNCTION AT FAULT ===");
        Function exact = getFunctionAt(a);
        Function containing = getFunctionContaining(a);
        println("  function at      : " + (exact == null ? "NONE" : exact.getName()));
        println("  function contain : " + (containing == null ? "NONE"
                : containing.getName() + " @ " + containing.getEntryPoint()
                  + " (fault is +0x" + Long.toHexString(a.subtract(containing.getEntryPoint())) + " in)"));
        if (containing != null) {
            println("  body range       : " + containing.getBody().getMinAddress()
                    + " - " + containing.getBody().getMaxAddress());
            println("  signature        : " + containing.getSignature().getPrototypeString());
        }

        println();
        println("=== NEAREST FUNCTIONS BELOW FAULT ===");
        long w = 0x2000;
        for (long p = offset - w; p <= offset + w; p += 4) {
            Address probe = toAddr(moduleBase + p);
            Function f = getFunctionAt(probe);
            if (f != null) {
                long rel = p - w;
                println(String.format("  0x%08X  rel +0x%06X  %s  body_end=%s",
                        moduleBase + p, rel, f.getName(), f.getBody().getMaxAddress()));
            }
        }

        println();
        println("=== RAW BYTES AT FAULT ===");
        Memory mem = currentProgram.getMemory();
        byte[] buf = new byte[64];
        Address start = toAddr(moduleBase + offset - 32);
        mem.getBytes(start, buf);
        for (int i = 0; i < buf.length; i += 16) {
            StringBuilder hex = new StringBuilder();
            StringBuilder asc = new StringBuilder();
            for (int j = 0; j < 16; j++) {
                int v = buf[i + j] & 0xFF;
                hex.append(String.format("%02x ", v));
                asc.append(v >= 32 && v < 127 ? (char) v : '.');
            }
            println(String.format("  %s  %s  |%s|",
                    start.add(i), hex, asc));
        }

        println();
        println("=== DISASSEMBLY AT FAULT ===");
        InstructionIterator ii = currentProgram.getListing().getInstructions(a, true);
        int n = 0;
        Address last = a;
        while (ii.hasNext() && n < 40) {
            Instruction ins = ii.next();
            String t = ins.toString();
            println("  " + ins.getAddress() + "  " + t);
            last = ins.getMaxAddress();
            if (t.startsWith("blr")) {
                println("  --> function appears to END at " + last
                        + "  (size = 0x" + Long.toHexString(last.subtract(a) + 1) + ")");
                break;
            }
            n++;
        }
        if (n == 0) {
            println("  (no instructions decoded at this address)");
        }

        println();
        println("=== MODULE STATS ===");
        println("  functions in module : " + fm.getFunctionCount());
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            println(String.format("  block %-10s %s - %s", b.getName(), b.getStart(), b.getEnd()));
        }
    }
}