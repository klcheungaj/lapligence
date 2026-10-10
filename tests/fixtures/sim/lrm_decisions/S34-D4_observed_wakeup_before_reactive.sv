// Decision S34-D4: a module process waiting for a clocking block event, which
// is triggered in the Observed region, resumes in the Active region before
// the reactive region set of the same pass runs; a program process waiting
// for the same event runs afterwards in the Reactive region. A synchronous
// drive the module process issues commits in the following Re-NBA region,
// so the program still reads the old value.
//
// IEEE 1800-2009 14.13 (SystemVerilog-1800-2009.txt L19921-19923):
//   "Upon processing its specified clocking event, a clocking block shall
//   update its sampled values before triggering the event associated with the
//   clocking block name. This event shall be triggered in the Observed
//   region."
// IEEE 1800-2009 4.5 (L3386-3392):
//   "while (any region in [Active ... Pre-Postponed] is nonempty) {
//      while (any region in [Active ... Post-Observed] is nonempty) {
//         execute_region (Active);
//         R = first nonempty region in [Active ... Post-Observed];
//         if (R is nonempty)
//            move events in R to the Active region;
//      }"
// IEEE 1800-2009 24.3.1 (L43248-43249):
//   "Statements and constructs within a program block that are sensitive to
//   changes (e.g., update events) on design signals are scheduled in the
//   Reactive region."
//
// llg ran the reactive set before design processes woken in Observed before.
module tb;
  bit clk = 0;
  logic [7:0] o = 0;
  clocking cb @(posedge clk);
    output o;
  endclocking
  program p;
    initial begin
      @(tb.cb);
      $display("program sees o=%0d", tb.o);
      #10;
    end
  endprogram
  initial begin
    @(cb);
    $display("module process woken by cb");
    cb.o <= 8'd7;
  end
  initial #5 clk = 1;
  always @(o) $display("o=%0d", o);
  initial #8 $finish;
endmodule
