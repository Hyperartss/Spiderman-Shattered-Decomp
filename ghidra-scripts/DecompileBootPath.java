// Decompile the boot-path functions to a file so they can be read directly.
// Output goes to the path in -postScript arg or stdout.

import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.Function;
import ghidra.program.model.address.Address;
import ghidra.util.task.ConsoleTaskMonitor;

import java.io.PrintWriter;
import java.io.FileWriter;

public class DecompileBootPath extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        String out = (args.length > 0) ? args[0] : null;

        long[] targets = {
            0x8218DD48L,   // calls XexLoadImage + XexGetProcedureAddress, then bctrl
            0x8218DF20L,   // its caller
            0x821FEEB8L,   // Ghidra: caller of 8218DD48
            0x82220370L,   // XexLoadImage wrapper
            0x82220330L,   // XexGetProcedureAddress wrapper
        };

        DecompInterface di = new DecompInterface();
        DecompileOptions opts = new DecompileOptions();
        di.setOptions(opts);
        di.openProgram(currentProgram);
        ConsoleTaskMonitor mon = new ConsoleTaskMonitor();

        PrintWriter w = (out != null) ? new PrintWriter(new FileWriter(out)) : new PrintWriter(System.out);

        for (long t : targets) {
            Address a = toAddr(t);
            Function f = getFunctionAt(a);
            if (f == null) f = getFunctionContaining(a);
            w.println("=====================================================================");
            w.println("FUNCTION " + (f != null ? f.getName() : "?") + " @ " + a);
            w.println("=====================================================================");
            if (f == null) { w.println("  (no function)"); continue; }

            w.println("caller(s):");
            for (Function c : f.getCallingFunctions(mon)) {
                w.println("   <- " + c.getName() + " @ " + c.getEntryPoint());
            }
            w.println();

            DecompileResults res = di.decompileFunction(f, 60, mon);
            if (res != null && res.decompileCompleted()) {
                w.println(res.getDecompiledFunction().getC());
            } else {
                w.println("  (decompile failed: " + (res == null ? "null result" : res.getErrorMessage()) + ")");
            }
            w.println();
        }

        di.dispose();
        if (out != null) { w.close(); println("written: " + out); }
    }
}