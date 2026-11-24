// Decision CS-D3: a released variable that continuous assignments or output
// ports drive holds the driven value when the release statement returns: a
// read in the same process, before it suspends, already sees it. The value
// is that of each driver at that moment: an assignment without a delay is
// evaluated with the present values of its operands (a blocking write made
// just before the release counts, a nonblocking one not yet), a delayed
// assignment gives the value it last delivered and an update still under
// way arrives at its time. A variable only procedures write keeps the
// forced value until its next procedural assignment.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13390-13393): "When
// released, then if the variable is not driven by a continuous assignment
// and does not currently have an active assign procedural continuous
// assignment, the variable shall not immediately change value and shall
// maintain its current value until the next procedural assignment to the
// variable is executed." L13393-13395: "Releasing a variable that is driven
// by a continuous assignment or currently has an active assign procedural
// continuous assignment shall reestablish that assignment and schedule a
// reevaluation in the continuous assignment's scheduling region." 4.7
// L3468-3470: "At any time while evaluating a procedural statement, the
// simulator may suspend execution and place the partially completed event
// as a pending event in the event region." The reevaluation may therefore
// run before the releasing process continues; that it does is the
// industry-standard behaviour. Variables computed from the released one by
// further continuous assignments are not read here: when they follow is not
// defined (4.7).
`timescale 1ns / 1ns
module stage (
    output logic [7:0] q,
    input  logic [7:0] d
);
  assign q = d + 8'd1;
endmodule

module tb;
  logic [7:0] in, ported, assigned, tied, parts, stored, delayed;
  logic [7:0] row[2];
  struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } pair;
  wire [7:0] net;
  stage u (
      .q(ported),
      .d(in)
  );
  assign assigned = in + 8'd2;
  assign tied = 8'd77;
  assign parts[3:0] = in[3:0];
  assign parts[7:4] = in[7:4] + 4'd1;
  assign pair.hi = in[3:0];
  assign pair.lo = in[7:4];
  assign row[0] = in + 8'd3;
  assign row[1] = in + 8'd4;
  assign #5 delayed = in + 8'd5;
  assign net = in + 8'd6;
  always @(in) stored = in + 8'd7;

  task show(input string label);
    $display("%s: ported=%0d assigned=%0d tied=%0d parts=%h pair=%h row=%0d,%0d stored=%0d net=%0d",
             label, ported, assigned, tied, parts, pair, row[0], row[1], stored, net);
  endtask

  task force_all;
    force ported = 8'd201;
    force assigned = 8'd202;
    force tied = 8'd203;
    force parts = 8'hee;
    force pair = 8'hdd;
    force row[1] = 8'd204;
    force stored = 8'd205;
    force net = 8'd206;
  endtask

  task release_all;
    release ported;
    release assigned;
    release tied;
    release parts;
    release pair;
    release row[1];
    release stored;
    release net;
  endtask

  initial begin
    in = 8'h21;
    #10 show("driven");
    // The operand changes while the forces hold.
    force_all;
    #1 in = 8'h43;
    #1 show("forced");
    release_all;
    show("at the release");
    #1 show("one unit later");
    // The operand changes in the releasing process just before the release.
    force assigned = 8'd202;
    #1 in = 8'h44;
    release assigned;
    $display("after a blocking write: assigned=%0d", assigned);
    // A delayed assignment: the delivered value, then the update under way.
    #10 force delayed = 8'd200;
    #1 in = 8'h10;
    #2 release delayed;
    $display("delayed at the release: %0d", delayed);
    #2 $display("delayed two units later: %0d", delayed);
    #2 $display("delayed after its delay: %0d", delayed);
    $finish;
  end
endmodule
