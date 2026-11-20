// A conditional real-array arm that is a function call: a located limit
// (docs/known_issues.md, real references and real-array expressions outside
// stable storage).
module tb;
  typedef real ra_t[3];
  function automatic ra_t f();
    f = '{1.5, 2.5, 3.5};
  endfunction
  ra_t a = '{0.5, 0.25, 0.125};
  ra_t r;
  bit c;
  initial begin
    c = 0;
    r = c ? a : f();
    $display("%g %g %g", r[0], r[1], r[2]);
    $finish(0);
  end
endmodule
