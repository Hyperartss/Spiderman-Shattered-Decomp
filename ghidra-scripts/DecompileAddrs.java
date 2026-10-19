// Decompile arbitrary guest addresses: pass them as script args.
//   -postScript DecompileAddrs.java 0x82207840 0x82202720 [out.txt]

import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.Function;
import ghidra.program.model.address.Address;
import ghidra.util.task.ConsoleTaskMonitor;

import java.io.PrintWriter;
import java.io.FileWriter;

public class DecompileAddrs extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) { println("usage: DecompileAddrs <addr...> [outfile]"); return; }

        String out = (args.length > 1 && !args[args.length - 1].startsWith("0x")) ? args[args.length - 1] : null;
        int n = out != null ? args.length - 1 : args.length;

        DecompInterface di = new DecompInterface();
        di.setOptions(new DecompileOptions());
        di.openProgram(currentProgram);
        ConsoleTaskMonitor mon = new ConsoleTaskMonitor();

        PrintWriter w = (out != null) ? new PrintWriter(new FileWriter(out)) : new PrintWriter(System.out);

        for (int i = 0; i < n; i++) {
            Address a = toAddr(Long.decode(args[i]));
            Function f = getFunctionAt(a);
            if (f == null) f = getFunctionContaining(a);
            w.println("=====================================================================");
            if (f == null) {
                w.println("NO FUNCTION at " + a);
                w.println("=====================================================================");
                continue;
            }
            w.println("FUNCTION " + f.getName() + " @ " + f.getEntryPoint());
            w.println("body: " + f.getBody().getMinAddress() + " - " + f.getBody().getMaxAddress());
            w.println("=====================================================================");
            w.println("callers:");
            for (Function c : f.getCallingFunctions(mon)) {
                w.println("   <- " + c.getName() + " @ " + c.getEntryPoint());
            }
            w.println();
            DecompileResults r = di.decompileFunction(f, 90, mon);
            if (r != null && r.decompileCompleted()) {
                w.println(r.getDecompiledFunction().getC());
            } else {
                w.println("(decompile failed: " + (r == null ? "null" : r.getErrorMessage()) + ")");
            }
            w.println();
        }

        di.dispose();
        if (out != null) { w.close(); println("written: " + out); }
    }
}