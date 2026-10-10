// SIM-014: Verilog-2001 repeated event controls (V2001 9.6, 9.7.7). A
// negative signed count runs zero times; an unsigned reg keeps its value;
// a blocking intra-assignment repeat captures its RHS first and resolves
// its destination when the control completes; a nonblocking one resolves
// both at issue.
`timescale 1ns / 1ns
module tb;
  event e;
  integer n;
  integer i;
  reg [1:0] u;
  reg [7:0] a, b, c;
  reg [7:0] m [0:3];

  // `e` occurs at 2, 4, 6, ...
  initial forever #2 ->e;

  initial begin
    m[0] = 0;
    m[2] = 0;
    #1 n = -3;
    repeat (n) @e;
    $display("%0t n=-3", $time);
    u = 2'b11;
    repeat (u) @e;
    $display("%0t u=3", $time);
    #1 b = 8'h12;
    n = 2;
    a = repeat (n) @e b;
    $display("%0t a=%h", $time, a);
    #1 i = 0;
    fork
      m[i] = @e 8'h5a;
      #1 i = 2;
    join
    #1 $display("%0t m0=%h m2=%h", $time, m[0], m[2]);
    $finish;
  end

  initial begin
    #3 c <= repeat (2) @e 8'h77;
    #4 $display("%0t c=%h", $time, c);
  end
endmodule
