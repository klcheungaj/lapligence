// SIM-043 A01: a VPI application (regions.c) traces its callbacks around
// Active, NBA, Observed, Reactive and Postponed activity of one time slot.
`timescale 1ns / 1ns
program prog;
  // The program outlives tb's $finish: completing every program would end
  // the simulation at time 5 (SV 24.3).
  initial begin
    #5 $display("[%0t] reactive: program", $time);
    #100;
  end
endprogram

module tb;
  logic [7:0] a = 0;
  logic [7:0] b = 0;
  logic [7:0] c = 0;
  logic [7:0] mem[0:3];
  logic clk = 0;
  clocking cb @(posedge clk);
  endclocking
  prog p ();
  always @(b) $display("[%0t] active: b=%0d", $time, b);
  always @(c) $display("[%0t] active: c=%0d", $time, c);
  initial begin
    @(cb);
    $display("[%0t] active: after observed clocking event", $time);
  end
  initial begin
    #5;
    $display("[%0t] active: initial", $time);
    a = 8'd1;
    $strobe("[%0t] postponed: strobe a=%0d b=%0d c=%0d", $time, a, b, c);
    #5;
    a = 8'd3;
    a = 8'd4;
    mem[2] = 8'h5a;
    mem[1] = 8'h11;
    #10 $display("[%0t] active: mem[2]=%h", $time, mem[2]);
    $finish;
  end
  initial begin
    #5;
    b <= 8'd2;
    clk <= 1'b1;
  end
endmodule
