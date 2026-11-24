// FIN-002 defect regression: a release through vpi_put_value (38.34) of a
// variable that a continuous assignment drives re-establishes the
// assignment, exactly as a release statement does (10.6.2): the variable
// holds the driven value when the call returns, without waiting for the
// next change of the assignment's operands. A variable only procedures
// write keeps the forced value until its next procedural assignment. The
// VPI application is shared_force_vpi.c.
`timescale 1ns / 1ns

module stage (
    input  logic [7:0] d,
    output logic [7:0] q
);
  assign q = d + 8'd1;
endmodule

module tb;
  logic [7:0] in;
  logic [7:0] assigned;
  logic [7:0] ported;
  logic [7:0] row[2];
  logic [7:0] both;
  logic [7:0] stored;
  logic [7:0] tied;
  struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } pair;

  assign assigned = in + 8'd2;
  stage u (
      .d(in),
      .q(ported)
  );
  assign row[0] = in + 8'd3;
  assign row[1] = in + 8'd4;
  assign both = in + 8'd5;
  assign tied = 8'd77;
  assign pair = in;
  always @(in) stored = in + 8'd6;

  task show(input string when);
    $display("[%0t] %s: assigned=%0d ported=%0d row=%0d,%0d both=%0d stored=%0d tied=%0d pair=%h",
             $time, when, assigned, ported, row[0], row[1], both, stored, tied, pair);
  endtask

  initial begin
    in = 8'd10;
    #1 show("driven");
    $vforce("tb.assigned", 101);
    $vforce("tb.ported", 102);
    $vforce("tb.row[1]", 103);
    $vforce("tb.both", 104);
    $vforce("tb.stored", 105);
    $vforce("tb.tied", 106);
    $vforce("tb.pair", 8'h5a);
    #1 show("forced");
    // The operands change while the forces hold.
    in = 8'd20;
    #1 show("forced, new operand");
    $vrelease("tb.assigned");
    $vrelease("tb.ported");
    $vrelease("tb.row[1]");
    $vrelease("tb.stored");
    $vrelease("tb.tied");
    $vrelease("tb.pair");
    // One variable is released by a statement and forced again through VPI.
    release both;
    show("at the releases");
    #1 show("released");
    $vforce("tb.both", 107);
    #1 show("forced again");
    $vrelease("tb.both");
    show("at the release");
    #1 show("released again");
    in = 8'd30;
    #1 show("next operand");
    $finish;
  end
endmodule
