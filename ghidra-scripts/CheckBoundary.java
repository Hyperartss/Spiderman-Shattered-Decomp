// Verify a guest function's boundary and entry-point validity.
//   -postScript CheckBoundary.java 0x82207840 [0x82202720 ...]
//
// Prints, per address: whether Ghidra sees a function, the body range, the
// next function start (to detect a gap or an overlap), and the first raw
// instructions decoded at the entry so the prologue can be judged by hand.

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;

public class CheckBoundary extends GhidraScript {

    @Override
    public void run() throws Exception {
        FunctionManager fm = currentProgram.getFunctionManager();

        for (String arg : getScriptArgs()) {
            Address a = toAddr(Long.decode(arg));
            println();
            println("=========================================================");
            println("ADDRESS " + a);
            println("=========================================================");

            Function f = getFunctionAt(a);
            if (f == null) {
                println("  Ghidra: NO FUNCTION at this address");
            } else {
                println("  function   : " + f.getName());
                println("  body       : " + f.getBody().getMinAddress() + " - " + f.getBody().getMaxAddress());
                println("  size       : 0x" + Long.toHexString(
                        f.getBody().getMaxAddress().subtract(f.getBody().getMinAddress()) + 1) + " bytes");
                println("  signature  : " + f.getSignature().getPrototypeString());
            }

            println();
            println("  --- first instructions decoded at entry ---");
            InstructionIterator ii = currentProgram.getListing().getInstructions(a, true);
            int n = 0;
            while (ii.hasNext() && n < 8) {
                Instruction ins = ii.next();
                String t = ins.toString();
                println("    " + ins.getAddress() + "  " + t);
                if (t.startsWith("blr")) break;
                n++;
            }
            if (n == 0) println("    (nothing decoded here)");

            println();
            println("  --- functions within +/-0x40 of entry ---");
            for (int d = -0x40; d <= 0x40; d += 4) {
                Address probe = a.add(d);
                Function g = getFunctionAt(probe);
                if (g != null) {
                    String off = Integer.toHexString(d);
                    if (d >= 0) off = "+" + off;
                    println("    " + off + "  " + g.getName()
                            + "  body_end=" + g.getBody().getMaxAddress());
                }
            }
        }
    }
}