`timescale 1ns/1ps
// Netlist-style cells: bused bit connections, escaped instance names,
// specify paths and a sequential UDP flop, eight instances of each cell so
// their port links, gates, path stamps and UDP gates share bodies.
primitive udp_dff (q, d, clk);
  output q;
  reg q;
  input d, clk;
  table
    // d clk : q : q+
       0  r  : ? : 0;
       1  r  : ? : 1;
       ?  f  : ? : -;
       *  ?  : ? : -;
  endtable
endprimitive

module dffc (input d, input clk, output q);
  udp_dff u (q, d, clk);
  specify
    (posedge clk => (q +: d)) = (1, 1);
  endspecify
endmodule

module inv (input a, output y);
  not g (y, a);
  specify
    (a => y) = (2, 3);
  endspecify
endmodule

module tb;
  reg clk = 1'b0;
  reg [7:0] din = 8'b1010_0110;
  wire [7:0] n;
  wire [7:0] q;
  inv \i_inv[0] (.a(din[0]), .y(n[0]));
  inv \i_inv[1] (.a(din[1]), .y(n[1]));
  inv \i_inv[2] (.a(din[2]), .y(n[2]));
  inv \i_inv[3] (.a(din[3]), .y(n[3]));
  inv \i_inv[4] (.a(din[4]), .y(n[4]));
  inv \i_inv[5] (.a(din[5]), .y(n[5]));
  inv \i_inv[6] (.a(din[6]), .y(n[6]));
  inv \i_inv[7] (.a(din[7]), .y(n[7]));
  dffc \r_reg[0] (.d(n[0]), .clk(clk), .q(q[0]));
  dffc \r_reg[1] (.d(n[1]), .clk(clk), .q(q[1]));
  dffc \r_reg[2] (.d(n[2]), .clk(clk), .q(q[2]));
  dffc \r_reg[3] (.d(n[3]), .clk(clk), .q(q[3]));
  dffc \r_reg[4] (.d(n[4]), .clk(clk), .q(q[4]));
  dffc \r_reg[5] (.d(n[5]), .clk(clk), .q(q[5]));
  dffc \r_reg[6] (.d(n[6]), .clk(clk), .q(q[6]));
  dffc \r_reg[7] (.d(n[7]), .clk(clk), .q(q[7]));
  initial begin
    #10 clk = 1'b1;
    #5 $display("%0t n=%b q=%b", $time, n, q);
    din = 8'b0000_1111;
    #5 clk = 1'b0;
    #5 clk = 1'b1;
    #3 $display("%0t n=%b q=%b", $time, n, q);
    $finish(0);
  end
endmodule
