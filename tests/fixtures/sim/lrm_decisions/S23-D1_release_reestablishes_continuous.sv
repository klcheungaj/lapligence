// Decision S23-D1: releasing a variable driven by a continuous assignment
// re-establishes that assignment; the reevaluation is scheduled, not done
// inside the release statement.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13393-13395):
//   "Releasing a variable that is driven by a continuous assignment or
//   currently has an active assign procedural continuous assignment shall
//   reestablish that assignment and schedule a reevaluation in the
//   continuous assignment's scheduling region."
//
// llg reruns every continuous driver of the released variable (a continuous
// assignment, a constant one, or an output port connection) as an ordinary
// Active-region event after the release. The case reads the variables one
// time unit later, so the order within the release time step does not matter.
module src_m (output logic [7:0] o, input logic [7:0] i);
  assign o = i;
endmodule

module tb;
  logic [7:0] src, cv, k, pi, pv;
  real rs, rv;
  assign cv = src;
  assign k = 8'h55;
  assign rv = rs * 2.0;
  src_m u (.o(pv), .i(pi));
  initial begin
    src = 8'h40; rs = 1.25; pi = 8'h01;
    force cv = 8'haa; force k = 8'h00; force rv = 0.5; force pv = 8'hcc;
    #1 $display("forced cv=%h k=%h rv=%g pv=%h", cv, k, rv, pv);
    src = 8'h41; rs = 2.0; pi = 8'h02;
    #1 $display("still forced cv=%h k=%h rv=%g pv=%h", cv, k, rv, pv);
    release cv; release k; release rv; release pv;
    #1 $display("released cv=%h k=%h rv=%g pv=%h", cv, k, rv, pv);
    src = 8'h42; pi = 8'h03;
    #1 $display("driven cv=%h pv=%h", cv, pv);
    $finish;
  end
endmodule
