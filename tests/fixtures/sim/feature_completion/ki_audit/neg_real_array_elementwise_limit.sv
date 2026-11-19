// An element-wise real-array expression above 4096 elements: a located
// limit (docs/known_issues.md, real references and real-array expressions
// outside stable storage).
module tb;
  real a[5000], b[5000], r[5000];
  bit c;
  initial begin
    a[4999] = 1.5;
    b[4999] = 2.5;
    c = 1;
    r = c ? a : b;
    $display("%g", r[4999]);
    $finish(0);
  end
endmodule
