// SV2009 3.12.1, 6.21, 10.5, 26.3: three declaration initializers in two
// files share one package counter. Their relative order is not defined across
// scopes, so the oracle checks order-independent results: the three callers
// receive 11, 12 and 13 in some order and every increment is retained.
module tb;
  int c1, c2;
  shared_user #(1) u1(c1);
  shared_user #(10) u2(c2);
  int mine = shared_pkg::take(1000);
  initial begin
    #1;
    $display("%0d %0d", c1 + c2 + mine, shared_pkg::hits);
    $display("%0d %0d", shared_pkg::take(0), shared_pkg::hits);
    $finish(0);
  end
endmodule
