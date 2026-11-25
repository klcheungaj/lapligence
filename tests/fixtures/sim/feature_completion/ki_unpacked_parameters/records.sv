// Copies, arguments and comparisons of structures with strings or reals that
// constant selectors reach in a parameter.
module m;
  typedef struct {int x; string s;} rec;
  typedef struct {int x; real r; rec n; int a[2]; logic [1:0] q[2];} all;
  parameter rec RC = '{7, "seven"};
  parameter rec RR [2] = '{'{1, "a"}, '{-2, "b"}};
  parameter all AL = '{3, 0.5, '{9, "n"}, '{4, 5}, '{1, 2'bx1}};
  rec r, q[$];
  all a;
  rec ra[2];
  function void show(rec v); $display("%0d %s", v.x, v.s); endfunction
  function automatic rec get(int k); rec t; t = RC; if (k) return RR[1]; return t; endfunction
  task automatic tk(input rec v, output rec w); w = v; #1; endtask
  class c; rec m; endclass
  initial #5 $display("%b %b %b %b", r == RR[0], r != RR[0], RC == r, RC == RR[1]);
  initial begin
    c h = new;
    r = RC; show(r);
    r = RR[1]; show(r);
    show(RR[0]);
    show(RC); show(RR[0]);
    r = get(0); show(r); r = get(1); show(r);
    tk(RC, r); show(r);
    a = AL; $display("%0d %0.2f %0d %s %0d %0d %b", a.x, a.r, a.n.x, a.n.s, a.a[0], a.a[1], a.q[1]);
    r = AL.n; show(r);
    q.push_back(RC); q.push_back(RR[1]); show(q[0]); show(q[1]);
    h.m = RC; show(h.m);
    ra[1] = RC; show(ra[1]);
    begin
      automatic rec l = RR[0];
      show(l);
      l = RC; show(l);
    end
  end
endmodule
module tb;
  m u ();
  m #(.RC('{8, "eight"})) v ();
endmodule
