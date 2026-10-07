// SIM-013 A03: always_comb versus @* (SV 9.2.2.2.1, 9.2.2.2.2, 9.4.2.2).
// always_comb is sensitive to reads inside called functions, @* only to the
// call's arguments; always_comb excludes storage it writes, including
// through a called function; references to class objects add nothing to an
// always_comb sensitivity list, so only the handle variable it reads wakes it.
`timescale 1ns / 1ns
module tb;
  class C;
    int x;
  endclass

  C h = new, h2 = new;
  int g = 0, a = 0, y_comb, y_star;
  int b = 0, t, z, scratch, n_comb = 0;
  int yc, n_cls = 0;

  function int rd(input int v);
    return v + g;
  endfunction

  function automatic int dbl(input int v);
    scratch = v * 2;
    return scratch;
  endfunction

  always_comb y_comb = rd(a);
  always @* y_star = rd(a);

  always_comb begin
    t = b + 1;
    t = t * 2;
    z = t + dbl(b);
    n_comb++;
  end

  always_comb begin
    yc = h.x;
    n_cls++;
  end

  initial h2.x = 7;

  initial begin
    #1 g = 5;
    #1 $display("%0t comb=%0d star=%0d", $time, y_comb, y_star);
    #1 a = 1;
    #1 $display("%0t comb=%0d star=%0d", $time, y_comb, y_star);
    #1 b = 3;
    #1 b = 3;
    #1 h.x = 2;
    #1 h = h2;
    #1 h2.x = 9;
    #1 $display("%0t z=%0d n_comb=%0d yc=%0d n_cls=%0d", $time, z, n_comb, yc, n_cls);
    $finish(0);
  end
endmodule
