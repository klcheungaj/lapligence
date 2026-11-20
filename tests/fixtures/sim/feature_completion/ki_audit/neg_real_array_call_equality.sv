// Equality of two real-array function results inside an expression: a
// located limit (docs/known_issues.md, real references and real-array
// expressions outside stable storage).
module tb;
  typedef real ra_t[3];
  function automatic ra_t f();
    f = '{1.0, 2.0, 3.0};
  endfunction
  function automatic ra_t g();
    g = '{1.0, 2.0, 3.0};
  endfunction
  initial begin
    int x;
    x = (f() == g()) ? 1 : 0;
    $display("%0d", x);
    $finish(0);
  end
endmodule
