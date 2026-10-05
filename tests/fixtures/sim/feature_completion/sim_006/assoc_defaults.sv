// SIM-006 A03: invalid associative indices and nonexistent entries
// (SV 7.8.6, Table 7-1, 7.9.11). A read through an X/Z index or of a missing
// entry warns and yields the element default; an invalid-index write is
// ignored with a warning; an explicit default is returned silently.
// Read-modify-write of a missing entry reads its default and creates it.
module tb;
  int a[int];
  string s[string];
  logic [7:0] w[*];
  int dflt[string];
  real r[int];
  logic [3:0] xk;
  int cnt[string];
  initial begin
    a[1] = 5;
    s["x"] = "y";
    xk = 4'bx01x;
    $display("missing=%0d", a[7]);
    $display("missing_s=[%s]", s["nope"]);
    $display("xidx=%0d", a[xk]);
    a[xk] = 9;
    $display("num=%0d", a.num());
    w[5] = 8'h11;
    $display("w=%h %h", w[5], w[6]);
    dflt = '{default: 42};
    $display("dflt=%0d num=%0d", dflt["any"], dflt.num());
    $display("real=%.1f", r[3]);
    cnt["k"] += 2;
    cnt["k"]++;
    a[1]--;
    $display("rmw=%0d %0d %0d", cnt["k"], cnt.num(), a[1]);
    $finish(0);
  end
endmodule
