`timescale 1ns/1ps
module tb;
  int a [2]; int w; int x;
  int v = 4; int fin = 0; reg clk = 0;
  initial begin localparam K = 1; $display("unnamed %0d", K + v); end
  initial begin : o
    localparam K = 10;
    begin : i localparam K2 = K + 1; localparam K = 100; $display("nested %0d %0d %0d", K, K2, o.K); end
    begin localparam K = 7; $display("nested unnamed %0d %0d", K, o.K); end
    for (int i = 0; i < 2; i++) begin : l localparam K = 3; $display("loop %0d", K * i); end
    foreach (a[i]) begin localparam Q = 9; $display("foreach %0d", Q + i); end
    repeat (1) begin localparam Q = 9; $display("repeat %0d", Q); end
    if (v) begin localparam Q = 11; $display("if %0d", Q); end else begin localparam Q = 12; $display("else %0d", Q); end
    case (v) 4: begin localparam Q = 13; $display("case %0d", Q); end endcase
  end
  initial #1 fork : f
    localparam F = 20;
    begin localparam G = F + 1; $display("fork a %0d %0d", F, G); end
    #1 $display("fork b %0d %0d", F, f.F);
  join
  initial #3 begin
    fork localparam H = 30; #1 $display("fork none %0d", H); join_none
    fork localparam H = 31; #2 $display("fork any %0d", H); join_any
  end
  always @(posedge clk) begin : al localparam D = 2; fin <= #D 7; $display("always %0d", D); end
  always_comb begin : ac localparam M = 8; w = v + M; end
  always_ff @(posedge clk) begin localparam M = 18; x <= v + M; end
  initial #10 begin $display("w %0d", w); clk = 1; #5 $display("fin %0d x %0d", fin, x); end
  final begin localparam Z = 77; $display("final %0d", Z); end
  generate for (genvar g = 0; g < 2; g++) begin : gg initial #20 begin : gb localparam GG = g * 2 + 1; $display("gen %0d %m", GG); end end endgenerate
  initial #30 begin $display("hier %0d %0d", o.K, gg[1].gb.GG); $display("hier2 %0d", tb.o.i.K); $display("%0d", tb.f.F); end
endmodule
