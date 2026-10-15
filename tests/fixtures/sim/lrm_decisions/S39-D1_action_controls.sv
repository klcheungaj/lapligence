// Decision S39-D1: the six action-control tasks gate only the execution of
// pass and fail actions; every attempt is still evaluated, and a later task
// changes only the actions of later results. $assertvacuousoff stays in
// force for vacuous successes after $assertnonvacuouson, until $assertpasson.
// llg also leaves its assertion statistics unchanged (not observable here).
//
// IEEE 1800-2009 20.12 (SystemVerilog-1800-2009.txt L35116-35121): "$assertpassoff
// shall stop execution of the pass action for vacuous and nonvacuous success
// ... the execution of pass action for only nonvacuous successes can be
// enabled subsequently by $assertnonvacuouson"; L35137-35139:
// "$assertvacuousoff shall stop execution of the pass action of all the
// specified assertions on vacuous success until a subsequent $assertpasson";
// L35160-35161: "These system tasks shall not affect the execution of pass or
// fail actions until the system task is executed. These system tasks shall
// not affect the statistics counters for the assertions."
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) b |-> a)
    $display("%0t pass", $time); else $display("%0t fail", $time);
  initial begin
    #8 $assertvacuousoff;        // 15: vacuous, not reported
    #12 a = 1'b1; b = 1'b1;      // 20
    #8 $assertpassoff;           // 28: 35 not reported
    #10 $assertnonvacuouson;     // 38: 45 reported
    #12 b = 1'b0;                // 50: 55 vacuous, not reported
    #8 $assertpasson;            // 58: 65 vacuous, reported
    #10 $assertfailoff;          // 68
    #2 a = 1'b0; b = 1'b1;       // 70: 75 fails, not reported
    #8 $assertfailon;            // 78: 85 fails, reported
    #12 $finish;                 // 90
  end
endmodule
