class base; int a; function new(int x=0); a=x; endfunction
  virtual function string nm(); return $sformatf("base%0d",a); endfunction endclass
class der extends base; function new(int x=0); super.new(x); endfunction
  virtual function string nm(); return $sformatf("der%0d",a); endfunction endclass
class hold;
  der q[$]; der fx[2]; der aa[string]; static der sq[$]; static der sv; der v; string sa[string]; int iq[$];
  der m[int];
  function bit put(int i, base b); return $cast(q[i], b); endfunction
  function void thisput(base b); if ($cast(this.fx[1], b)) $display("this ok"); $cast(sv, b); $cast(this.v, b); endfunction
endclass
typedef struct { der h; der arr[2]; string s; int n; } rec_t;
module tb;
  function automatic base mk(int k); der d; base b; if (k==0) return null; if (k==1) begin d=new(7); return d; end b=new(8); return b; endfunction
  der nest[string][int]; der qq[$][$]; string ss[string][$]; der m2[2][3]; rec_t rq[$]; rec_t ra[string]; rec_t rf[2]; rec_t r;
  hold hq[$];
  hold hh = new;
  function automatic void autof(ref der rq[$], input base b); der lq[$]; der lf[2]; lq.push_back(null); $cast(lq[0], b); $cast(lf[1], b); $cast(rq[0], b); $display("%s %s %s", lq[0].nm(), lf[1].nm(), rq[0].nm()); endfunction
  task automatic tk(ref der rq[$], input base b); der lq[$]; #1; lq.push_back(null); $cast(lq[0], b); #1; $display("%s", lq[0].nm()); endtask
  function hold geth(); $display("geth"); return hh; endfunction
  initial begin
    int i; der lq[$];
    $display("%0d", $cast(nest["a"][1], mk(1))); $display("%s", nest["a"][1].nm());
    qq.push_back(lq); qq[0].push_back(null);
    $display("%0d", $cast(qq[0][0], mk(1))); $display("%s", qq[0][0].nm());
    ss["k"].push_back("x");
    $display("%0d", $cast(ss["k"][0], "yy")); $display("%s", ss["k"][0]);
    $display("%0d", $cast(m2[1][2], mk(1))); $display("%s", m2[1][2].nm());
    begin rec_t e; rq.push_back(e); end
    $display("%0d", $cast(rq[0].h, mk(1))); $display("%s", rq[0].h.nm());
    $display("%0d", $cast(ra["x"].h, mk(1))); $display("%s %0d", ra["x"].h.nm(), ra.size());
    $display("%0d", $cast(rf[1].h, mk(1))); $display("%s", rf[1].h.nm());
    $display("%0d", $cast(r.h, mk(1))); $display("%s", r.h.nm());
    $display("%0d", $cast(r.arr[1], mk(1))); $display("%s", r.arr[1].nm());
    $display("%0d", $cast(rq[0].arr[0], mk(1))); $display("%s", rq[0].arr[0].nm());
    $display("%0d", $cast(rq[0].s, "st")); $display("%s", rq[0].s);
    $display("%0d", $cast(rq[0].n, 5)); $display("%0d", rq[0].n);
    $display("%0d", hh.put(0, mk(1))); $display("%s", hh.q[0].nm());
    hh.thisput(mk(1)); $display("%s %s %s", hh.fx[1].nm(), hold::sv.nm(), hh.v.nm());
    $display("%0d", $cast(hh.q[0], mk(1)));
    $display("%0d", $cast(hh.aa["k"], mk(1))); $display("%s", hh.aa["k"].nm());
    $display("%0d", $cast(hold::sq[0], mk(1))); $display("%0d", hold::sq.size());
    $display("%0d", $cast(hh.sa["k"], "v")); $display("%s", hh.sa["k"]);
    $display("%0d", $cast(geth().q[0], mk(1)));
    $display("%0d", $cast(geth().m[3], mk(2))); $display("%0d", hh.m.size());
    hq.push_back(hh);
    $display("%0d", $cast(hq[0].q[1], mk(1))); $display("%0d", hh.q.size());
    $display("%0d", $cast(hh.q[$], mk(1)));
    lq.push_back(null); lq.push_back(null);
    autof(lq, mk(1));
    tk(lq, mk(1));
    i = 0;
    while ($cast(lq[i], mk(1))) begin i++; if (i==4) break; end
    $display("%0d %0d", i, lq.size());
    for (i=0; i<2 && !$cast(lq[i], mk(2)); i++); $display("%0d", i);
    $display("%0d", $cast(lq[0], mk(1)) ? 10 : 20);
    if ($cast(lq[0], mk(1)) && lq[0].a == 7) $display("and");
  end
  initial begin : dyn
    der dd[]; automatic der ad[] = new[2];
    fork begin $cast(ad[0], mk(1)); #1; $display("%s", ad[0].nm()); end join
  end
endmodule
