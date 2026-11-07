class base; int a; function new(int x=0); a=x; endfunction endclass
class der extends base; function new(int x=0); super.new(x); endfunction endclass
typedef struct { base h; int n; } rec_t;
class h2; base f; base q[$]; base aa[string]; base fx[2]; static base sv; static base sq[$]; endclass
module tb;
  function automatic void out1(output base b, input int x); b = new(x); endfunction
  function automatic void ref1(ref base b, input int x); b = new(x); endfunction
  function automatic base ret1(output base b, input int x); b = new(x); return b; endfunction
  function automatic void inout1(inout base b); b = new(b==null ? 1 : b.a+1); endfunction
  task automatic tout(output base b, input int x); #1; b = new(x); endtask
  task automatic tref(ref base b, input int x); #1; b = new(x); endtask
  base q[$]; base da[]; base aa[int]; base sa[string]; base fx[3]; base m[2][2]; rec_t r; rec_t rq[$]; h2 h = new;
  base v;
  initial begin
    q={null,null}; da=new[2]; begin rec_t e; rq.push_back(e); end
    out1(q[0], 1); $display("%0d", q[0].a);
    out1(da[1], 2); $display("%0d", da[1].a);
    out1(aa[5], 3); $display("%0d %0d", aa[5].a, aa.size());
    out1(sa["k"], 4); $display("%0d", sa["k"].a);
    out1(fx[1], 5); $display("%0d", fx[1].a);
    out1(m[1][0], 6); $display("%0d", m[1][0].a);
    out1(r.h, 7); $display("%0d", r.h.a);
    out1(rq[0].h, 8); $display("%0d", rq[0].h.a);
    out1(h.f, 9); $display("%0d", h.f.a);
    h.q.push_back(null);
    out1(h.q[0], 10); $display("%0d", h.q[0].a);
    out1(h.aa["x"], 11); $display("%0d", h.aa["x"].a);
    out1(h.fx[1], 12); $display("%0d", h.fx[1].a);
    out1(h2::sv, 13); $display("%0d", h2::sv.a);
    out1(q[2], 14); $display("%0d", q.size());
    out1(q[9], 15); $display("%0d", q.size());
    ref1(q[0], 21); $display("%0d", q[0].a);
    ref1(da[1], 22); $display("%0d", da[1].a);
    ref1(aa[5], 23); $display("%0d", aa[5].a);
    ref1(sa["k"], 24); $display("%0d", sa["k"].a);
    ref1(fx[1], 25); $display("%0d", fx[1].a);
    ref1(m[1][0], 26); $display("%0d", m[1][0].a);
    ref1(r.h, 27); $display("%0d", r.h.a);
    ref1(h.f, 29); $display("%0d", h.f.a);
    ref1(h.q[0], 30); $display("%0d", h.q[0].a);
    ref1(h.aa["x"], 31); $display("%0d", h.aa["x"].a);
    ref1(h.fx[1], 32); $display("%0d", h.fx[1].a);
    ref1(h2::sv, 33); $display("%0d", h2::sv.a);
    v = ret1(q[0], 41); $display("%0d %0d", q[0].a, v.a);
    v = ret1(fx[2], 42); $display("%0d %0d", fx[2].a, v.a);
    v = ret1(aa[7], 43); $display("%0d %0d", aa[7].a, v.a);
    inout1(q[0]); $display("%0d", q[0].a);
    inout1(fx[0]); $display("%0d", fx[0].a);
    inout1(sa["k"]); $display("%0d", sa["k"].a);
    tout(q[0], 51); $display("%0d", q[0].a);
    tout(aa[8], 52); $display("%0d", aa[8].a);
    tout(fx[1], 53); $display("%0d", fx[1].a);
    tref(q[0], 54); $display("%0d", q[0].a);
    tref(fx[1], 55); $display("%0d", fx[1].a);
  end
endmodule
