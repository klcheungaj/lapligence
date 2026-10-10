// Decision S36-D5: $assertkill flushes only reports that have not matured.
//
// IEEE 1800-2009 20.11 (SystemVerilog-1800-2009.txt L35054-35057):
//   "$assertkill shall abort execution of any currently executing specified
//   assertions and then stop the checking of all specified assertions until
//   a subsequent $asserton. This also flushes any queued pending reports of
//   deferred assertions (see 16.4) ... that have not yet matured."
// 16.4.1 (L21263-21264): "Once a report matures, it may no longer be
// flushed."
//
// When an action executing in the Reactive region calls $assertkill, the
// other matured reports of that time step still execute; llg treats a
// matured report that has not started as neither pending nor "currently
// executing".
module tb;
  task killer();
    $display("killer");
    $assertkill;
  endtask
  initial begin
    a1: assert #0 (1'b0) else killer();
    a2: assert #0 (1'b0) else $display("a2 matured, still runs");
    #1 a3: assert #0 (1'b0) else $display("not printed: checking killed");
    #1 $finish;
  end
endmodule
