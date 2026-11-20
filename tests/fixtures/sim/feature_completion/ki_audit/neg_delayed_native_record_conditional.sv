// A blocking #delay assignment of a record with a string member and a
// conditional source: a located limit (docs/known_issues.md, delayed native
// continuous writes).
module tb;
  typedef struct { int a; string s; } rec_t;
  rec_t x, y, r;
  bit c;
  initial begin
    x = '{1, "x"};
    y = '{2, "y"};
    c = 1;
    r = #1 (c ? x : y);
    $display("%0t %0d %s", $time, r.a, r.s);
    $finish(0);
  end
endmodule
