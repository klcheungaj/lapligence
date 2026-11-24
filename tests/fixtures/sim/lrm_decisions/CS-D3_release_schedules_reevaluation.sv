// Decision CS-D3: releasing a variable that a continuous assignment (or an
// output port) drives schedules the assignment's reevaluation; the release
// statement itself does not change the variable. A read in the same
// process before it suspends still sees the forced value, and the driven
// value is there once the assignment has run in its region. A released net
// takes its drivers' value at once.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13393-13395):
// "Releasing a variable that is driven by a continuous assignment or
// currently has an active assign procedural continuous assignment shall
// reestablish that assignment and schedule a reevaluation in the continuous
// assignment's scheduling region." L13398-13399: "When released, the net
// shall immediately be assigned the value determined by the drivers of the
// net." Some simulators show the driven value of the variable to the read
// that follows the release statement; llg follows the clause text, which
// schedules the reevaluation of a variable and assigns only a net
// immediately.
module stage (
    output logic [7:0] q,
    input  logic [7:0] d
);
  assign q = d + 8'd1;
endmodule

module tb;
  logic [7:0] in, ported, assigned;
  wire [7:0] net;
  stage u (
      .q(ported),
      .d(in)
  );
  assign assigned = in + 8'd2;
  assign net = in + 8'd3;
  initial begin
    in = 8'd1;
    #1 force ported = 8'd99;
    force assigned = 8'd98;
    force net = 8'd97;
    #1 release ported;
    release assigned;
    release net;
    $display("at the release: ported=%0d assigned=%0d net=%0d", ported, assigned, net);
    #1 $display("one unit later: ported=%0d assigned=%0d net=%0d", ported, assigned, net);
    $finish;
  end
endmodule
