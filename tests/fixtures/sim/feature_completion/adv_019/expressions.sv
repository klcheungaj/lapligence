// ADV-019 family 3: VPI expression objects (IEEE 1800-2009 37.54, 37.55,
// 37.19, 37.38), walked by expressions.c.
`timescale 1ns/1ns
module leaf (input logic [7:0] a, output logic [7:0] y);
  assign y = a;
endmodule

module tb;
  logic [7:0] a = 8'h5a, b = 8'h0f;
  logic [3:0] n = 4'b10x1;
  logic signed [7:0] s = -8'sd3;
  int i = 2;
  real r = 1.5;
  logic [7:0] mem [0:3];
  logic [15:0] w = 16'hbeef;
  logic [0:3][7:0] pk = {8'h01, 8'h02, 8'h03, 8'h04};
  function automatic int twice(input int v);
    return 2 * v;
  endfunction
  logic [7:0] e_sum, e_ter, e_ips, e_cat, e_neg, e_mem, e_pk;
  logic [15:0] e_mul, e_pkr;
  logic e_bit, e_wild, e_log, e_not;
  real e_real;
  int e_cast, e_call, e_clog;
  assign e_sum = a + b * 2;
  assign e_mul = (a + b) * (a - b);
  assign e_cat = {a[3:0], {2{n[1:0]}}};
  assign e_ter = a > b ? a - b : b - a;
  assign e_ips = w[i * 4 +: 8];
  assign e_neg = -s >>> 1;
  assign e_mem = mem[i][3:0];
  assign e_pk = pk[1];
  assign e_pkr = pk[i -: 2];
  assign e_bit = w[17];
  assign e_wild = a ==? 8'b0101_xxxx;
  assign e_log = i < 3 && r > 1.0;
  assign e_not = !n;
  assign e_real = r * 2.0;
  assign e_cast = int'(r) + 8'shf0;
  assign e_call = twice(3);
  assign e_clog = $clog2(i);
  leaf u (.a(a ^ b), .y());
  initial begin
    mem[0] = 8'h11; mem[1] = 8'h22; mem[2] = 8'h33; mem[3] = 8'h44;
    #1 $exprs(~a & b, r / 4, "hi");
    a = 8'h01;
    i = 1;
    #1 $again;
    $finish;
  end
endmodule
