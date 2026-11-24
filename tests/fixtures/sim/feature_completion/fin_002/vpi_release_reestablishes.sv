// FIN-002 defect regression: a release through vpi_put_value (38.34) of a
// variable that a continuous assignment drives re-establishes the
// assignment (10.6.2) without waiting for the next change of its operands.
// A port connection that copies a variable and a constant assignment give
// their value before vpi_put_value returns; an assignment of an expression
// is scheduled and has delivered its value when the time step ends, in a
// single instance or in one of many that share a generated body, and also
// when a release statement elsewhere names the same variable. A variable
// only procedures write keeps the forced value until its next procedural
// assignment. The VPI application is shared_force_vpi.c. See readme.md.
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
  // Six instances share one generated driver body.
  logic [7:0] seed[6];
  for (genvar i = 0; i < 6; i++) begin : g
    logic [7:0] q;
    stage s (
        .d(seed[i]),
        .q(q)
    );
  end

  task show(input string when);
    $display("[%0t] %s: assigned=%0d ported=%0d row=%0d,%0d both=%0d stored=%0d tied=%0d pair=%h",
             $time, when, assigned, ported, row[0], row[1], both, stored, tied, pair);
    $display("[%0t] %s: s.q=%0d,%0d,%0d", $time, when, g[1].s.q, g[4].s.q, g[5].s.q);
  endtask

  // The same values once every scheduled reevaluation of the time step ran.
  task at_end(input string when);
    $strobe("[%0t] %s: assigned=%0d ported=%0d row=%0d,%0d both=%0d stored=%0d tied=%0d pair=%h",
            $time, when, assigned, ported, row[0], row[1], both, stored, tied, pair);
    $strobe("[%0t] %s: s.q=%0d,%0d,%0d", $time, when, g[1].s.q, g[4].s.q, g[5].s.q);
  endtask

  initial begin
    in = 8'd10;
    for (int unsigned i = 0; i < 6; i++) seed[i] = 8'(40 + i);
    #1 show("driven");
    $vforce("tb.assigned", 101);
    $vforce("tb.ported", 102);
    $vforce("tb.row[1]", 103);
    $vforce("tb.both", 104);
    $vforce("tb.stored", 105);
    $vforce("tb.tied", 106);
    $vforce("tb.pair", 8'h5a);
    $vforce("tb.g[1].s.q", 111);
    $vforce("tb.g[4].s.q", 114);
    #1 show("forced");
    // The operands change while the forces hold.
    in = 8'd20;
    for (int unsigned i = 0; i < 6; i++) seed[i] = 8'(60 + i);
    #1 show("forced, new operand");
    $vrelease("tb.assigned");
    $vrelease("tb.ported");
    $vrelease("tb.row[1]");
    $vrelease("tb.stored");
    $vrelease("tb.tied");
    $vrelease("tb.pair");
    $vrelease("tb.g[4].s.q");
    // One variable is released by a statement and forced again through VPI.
    release both;
    show("at the releases");
    at_end("end of the time step");
    #1 show("released");
    $vforce("tb.both", 107);
    #1 show("forced again");
    $vrelease("tb.both");
    $vrelease("tb.g[1].s.q");
    show("at the release");
    at_end("end of the time step");
    #1 show("released again");
    in = 8'd30;
    for (int unsigned i = 0; i < 6; i++) seed[i] = 8'(80 + i);
    #1 show("next operand");
    $finish;
  end
endmodule
