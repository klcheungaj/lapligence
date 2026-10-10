// Decision S36-D6: deferred assertion reports still queued when $finish
// executes run before the final procedures.
//
// IEEE 1800-2009 20.2 (SystemVerilog-1800-2009.txt L34119): "The $finish
// system task causes the simulator to exit and pass control back to the host
// operating system."
// 16.4.1 (L21262-21265): pending reports mature in the Observed region and
// execute in the Reactive region of the time step.
//
// The text does not say whether reports queued in the time step that calls
// $finish execute. llg executes them, in issue order, before the final
// procedures (unlike Postponed $strobe/$monitor output, decision S25-D5). A
// simulator that exits at once would print only "before finish" and
// "final".
module tb;
  initial begin
    assert #0 (1'b0) else $display("pending at finish");
    $display("before finish");
    $finish;
  end
  final $display("final");
endmodule
