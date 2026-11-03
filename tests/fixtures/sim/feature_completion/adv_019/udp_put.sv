// ADV-019 vpi_put_value to sequential UDPs (IEEE 1800-2009 38.34, decision
// A19-D74) across the forms a gate takes: an undelayed and a delayed
// instance, elements of a UDP instance array, instances of a generate loop
// (one shared body), a definition too wide for the dense next-state index,
// a put before simulation starts and a put during the instance's own input
// event. Expected output: readme.md.
`timescale 1ns / 1ns
primitive up_dff (q, d, clk);
  output q;
  reg q;
  input d, clk;
  initial q = 0;
  table
    0 r : ? : 0;
    1 r : ? : 1;
    ? (?0) : ? : -;
    * ? : ? : -;
  endtable
endprimitive

// Ten inputs with transitions on four of them: 3^11 * 9 next-state entries
// exceed the dense index, so its instances step through the row scan.
primitive up_wide (q, c, d, e1, e2, e3, e4, e5, e6, e7, e8);
  output q;
  reg q;
  input c, d, e1, e2, e3, e4, e5, e6, e7, e8;
  table
    r 0 ? ? ? ? ? ? ? ? : ? : 0;
    r 1 ? ? ? ? ? ? ? ? : ? : 1;
    (?0) ? ? ? ? ? ? ? ? ? : ? : -;
    ? * ? ? ? ? ? ? ? ? : ? : -;
    ? ? * ? ? ? ? ? ? ? : ? : -;
    ? ? ? * ? ? ? ? ? ? : ? : -;
  endtable
endprimitive

primitive up_inv (q, a);
  output q;
  input a;
  table
    0 : 1;
    1 : 0;
  endtable
endprimitive

module tb;
  reg d = 0, clk = 0;
  wire qs, qn, qd, qw, qi;
  wire [1:0] qa;
  wire [3:0] qg;
  up_dff us (qs, d, clk);
  up_dff un (qn, d, clk);
  up_dff #4 ud (qd, d, clk);
  up_dff #(2, 3) ua [1:0] (qa, d, clk);
  up_wide #1 uw (qw, clk, d, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0);
  up_inv ui (qi, d);
  genvar k;
  for (k = 0; k < 4; k = k + 1) begin : g
    up_dff #3 u (qg[k], d, clk);
  end
  always @(qs, qn, qd, qa, qw, qg)
    $display("%0t qs=%b qn=%b qd=%b qa=%b qw=%b qg=%b", $time, qs, qn, qd, qa, qw, qg);
  initial begin
    #10 $udp_put(1);
    #10 d = 1'b1;
    clk = 1'b1;
    #1 $udp_put(2);
    #9 clk = 1'b0;
    #10 $udp_put(3);
    #10 $udp_put(4);
    #5 d = 1'b0;
    #5 clk = 1'b1;
    #10 $udp_put(5);
    #10 $finish;
  end
endmodule
