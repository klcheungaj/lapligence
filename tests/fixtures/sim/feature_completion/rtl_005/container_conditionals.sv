// FIN-001 G1: the conditional operator with queue, dynamic and associative
// operands (SV 11.4.11; decision R05-D2).
module tb;
  typedef struct { int n; string s; } rec_t;
  logic c;
  int qa[$], qb[$], qr[$];
  int bounded[$:2];
  logic [7:0] da[], db[], dr[];
  int aa[string], ab[string], ar[string];
  rec_t ra[$], rb[$], rr[$];
  int nested_a[$][$], nested_b[$][$], nested_r[$][$];
  int calls;
  function automatic int total(int v[$]);
    int t = 0;
    foreach (v[i]) t += v[i];
    return t;
  endfunction
  typedef int iq_t[$];
  function automatic iq_t get(iq_t v);
    calls++;
    return v;
  endfunction
  initial begin
    qa = '{1, 6, 3};
    qb = '{1, 9, 3, 8};
    da = new[3];
    db = new[2];
    da[0] = 8'h0f; da[1] = 8'h11; da[2] = 8'h22;
    db[0] = 8'hf0; db[1] = 8'h11;
    aa["k"] = 1; aa["m"] = 2;
    ab["k"] = 1; ab["m"] = 13; ab["n"] = 4;
    ra = '{'{1, "a"}, '{2, "b"}};
    rb = '{'{1, "a"}, '{13, "b"}, '{5, "c"}};
    nested_a.push_back(qa);
    nested_a.push_back(qb);
    nested_b.push_back(qb);
    // Known conditions select one whole arm.
    c = 1'b1;
    qr = c ? qa : qb;
    dr = c ? db : da;
    ar = c ? ab : aa;
    $display("t %0d %0d %0d | %0d %h | %0d %0d", qr.size(), qr[1], qr[2],
             dr.size(), dr[0], ar.size(), ar["m"]);
    c = 1'b0;
    qr = c ? qa : qb;
    rr = c ? ra : rb;
    $display("f %0d %0d | %0d %0d %s", qr.size(), qr[3], rr.size(), rr[2].n,
             rr[2].s);
    // An ambiguous condition merges element by element.
    c = 1'bx;
    qr = c ? qa : qb;
    $display("xq %0d: %0d %0d %0d %0d", qr.size(), qr[0], qr[1], qr[2], qr[3]);
    dr = c ? da : db;
    $display("xd %0d: %h %h %h", dr.size(), dr[0], dr[1], dr[2]);
    ar = c ? aa : ab;
    $display("xa %0d: %0d %0d %0d", ar.size(), ar["k"], ar["m"], ar["n"]);
    rr = c ? ra : rb;
    $display("xr %0d: %0d%s %0d%s %0d[%s]", rr.size(), rr[0].n, rr[0].s,
             rr[1].n, rr[1].s, rr[2].n, rr[2].s);
    nested_r = c ? nested_a : nested_b;
    $display("xn %0d: %0d %0d | %0d %0d %0d %0d", nested_r.size(),
             nested_r[0].size(), nested_r[1].size(), nested_r[1][0],
             nested_r[1][1], nested_r[1][2], nested_r[1][3]);
    bounded = c ? qa : qb[0:1];
    $display("xb %0d: %0d %0d %0d", bounded.size(), bounded[0], bounded[1],
             bounded[2]);
    c = 1'bz;
    $display("xt %0d", total(c ? qa : qb));
    // Each reached arm runs once.
    calls = 0;
    qr = c ? get(qa) : get(qb);
    $display("arms %0d %0d", qr.size(), calls);
    c = 1'b0;
    calls = 0;
    qr = c ? get(qa) : get(qb);
    $display("arms %0d %0d", qr.size(), calls);
    $finish(0);
  end
endmodule
