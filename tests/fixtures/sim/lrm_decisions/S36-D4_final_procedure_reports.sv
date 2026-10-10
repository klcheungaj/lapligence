// Decision S36-D4: a deferred assertion in a final procedure reports when
// that final procedure returns.
//
// IEEE 1800-2009 9.2.3 (SystemVerilog-1800-2009.txt L11256-11259): "the
// final procedure does not execute as a separate process; instead, it
// executes in zero time, as a series of function calls from a single
// process. ... No remaining scheduled events shall execute after all final
// procedures have executed."
// 16.4.1 (L21262-21265): pending reports mature "In the Observed region of
// each simulation time step"; no Observed region follows the final
// procedures.
//
// The text does not say what happens to a report queued by a final
// procedure. llg executes it (with its issue-time arguments) when the final
// procedure that issued it returns, before any other final procedure.
module tb;
  int v = 3;
  final begin
    assert #0 (v == 4) else $display("final fail v=%0d", v);
    v = 5;
    $display("final end v=%0d", v);
  end
  initial #1 $finish;
endmodule
