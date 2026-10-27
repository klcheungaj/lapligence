// Decision A20-D10: cbDisable. A record runs inside the `disable` statement,
// right after the named block or task it names was disabled and before the
// disabling process continues (for a block that disables itself, before it
// unwinds). It runs only when the statement ended an active block or task: a
// disable of a block that is not running runs no callback. A record on a
// block runs only when `disable` names that block, not when an enclosing
// block is disabled; a record on a system task call runs when any named
// block enclosing the call is disabled. The callback gets the registered
// object and a NULL value. Registering cbDisable without an object, or on a
// variable or a module, fails with vpiError.
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L64648-L64649):
// "cbDisable After a named block or task containing a system task or system
// function has been disabled."; L64785-L64786: "For a cbDisable callback, obj
// shall be a handle to a system task call, system function call, named begin,
// named fork, task, or function." The time of the callback, inactive blocks,
// enclosing blocks and the NULL value are llg choices.
//
// Build A20-D10_disable_callback.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d10()` and `task $d10_call()` declared as system
// tasks).
`timescale 1ns/1ns
module tb;
    int v;
    initial begin
        $d10;
        fork
            begin : outer
                begin : inner
                    $d10_call;
                    #10 $display("inner finished");
                end
            end
            begin
                #1 disable outer;
                $display("after disable outer at %0t", $time);
                #1 disable outer;
                $display("after second disable at %0t", $time);
            end
        join
        begin : selfb
            disable selfb;
            $display("selfb continued");
        end
        $display("done at %0t", $time);
        $finish;
    end
endmodule
