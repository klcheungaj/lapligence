// Decision CS-D5: a released variable that a delayed continuous assignment
// drives takes the value that assignment last delivered when the release
// statement returns, and an update still under way arrives at its time. A
// delayed assignment to selected bits gives those bits only; the other bits
// follow their own drivers (CS-D3) or, undriven, keep the forced value.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13393-13395):
// "Releasing a variable that is driven by a continuous assignment or
// currently has an active assign procedural continuous assignment shall
// reestablish that assignment and schedule a reevaluation in the continuous
// assignment's scheduling region." 10.3.3 L12894-12895: "A delay given to a
// continuous assignment shall specify the time duration between a
// right-hand operand value change and the assignment made to the left-hand
// side." No operand changes at the release, so the reestablished assignment
// holds what it last assigned; the value an operand change put under way
// before the release is assigned when its delay has passed. That the
// variable holds the delivered value at once, like an assignment without a
// delay (CS-D3), is the industry-standard behaviour.
`timescale 1ns / 1ns
module tb;
  logic [7:0] in, whole, parts, mixed, half;
  logic [7:0] row[2];
  assign #5 whole = in + 8'd5;
  assign #3 parts[3:0] = in[3:0];
  assign #6 parts[7:4] = in[7:4];
  assign #3 mixed[3:0] = in[3:0];
  assign mixed[7:4] = in[7:4];
  assign #5 half[7:4] = in[3:0];
  assign #5 row[1] = in + 8'd1;

  task show(input string label);
    $display("%s: whole=%h parts=%h mixed=%h half=%h row=%h", label, whole, parts, mixed, half,
             row[1]);
  endtask

  task force_all;
    force whole = 8'hee;
    force parts = 8'hee;
    force mixed = 8'hee;
    force half = 8'hee;
    force row[1] = 8'hee;
  endtask

  task release_all;
    release whole;
    release parts;
    release mixed;
    release half;
    release row[1];
  endtask

  initial begin
    in = 8'h21;
    #10 show("driven");
    // Updates under way at the release: they arrive at 14, 16 and 17.
    force_all;
    #1 in = 8'h43;
    #2 release_all;
    show("at the release");
    #2 show("two units later");
    #3 show("five units later");
    // Updates that matured while the forces held.
    #12 force_all;
    #1 in = 8'h65;
    #9 show("forced");
    release_all;
    show("at the second release");
    #1 show("one unit later");
    $finish;
  end
endmodule
