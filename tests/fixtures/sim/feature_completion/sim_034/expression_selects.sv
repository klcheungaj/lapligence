`timescale 1ns/1ns
// SV 1800-2009 14.16: selects of clockvars bound to a part-select of a
// variable (the clockvar keeps the selected range, decision K2-D1), to an
// unpacked array element, and of a clockvar whose signal is a wire with
// another driver (FIN-001 K2).
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  logic [7:0] s;
  logic [3:0] mem [2];
  wire [3:0] n;
  logic [3:0] o = 4'b00zz;
  assign n = o;
  clocking cb @(posedge clk);
    output x = s[5:2];
    output e = mem[1];
    output n;
  endclocking
  initial begin
    s = 8'h00;
    mem[1] = 4'h0;
    #6 cb.x[3] <= 1'b1;
    cb.e <= 4'h9;
    cb.n[1:0] <= 2'b10;
    #10 $display("%0t s=%b mem1=%b n=%b", $time, s, mem[1], n);
    cb.x[5:4] <= 2'b11;
    cb.e[2] <= 1'b1;
    cb.n[3] <= 1'b1;
    #10 $display("%0t s=%b mem1=%b n=%b", $time, s, mem[1], n);
    o = 4'bzzzz;
    #1 $display("%0t n=%b", $time, n);
    $finish;
  end
endmodule
