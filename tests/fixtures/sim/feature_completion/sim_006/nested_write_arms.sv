class H; int v; endclass
module tb;
  typedef struct { int a; string s; } rec_t;
  typedef struct { int a; string s; int q[$]; } rq_t;
  rec_t ra[string][int];
  rq_t cr[string][int];
  rec_t rq[string][$];
  string ss[string][int];
  H hh[string][int];
  int nq[string][$];
  int v;
  bit c = 1;
  bit u;
  logic x = 1'bx;
  rec_t e;
  rq_t ce;
  H he = null;
  function automatic rec_t g(rec_t in); rec_t r = in; r.a++; ra.delete("x"); return r; endfunction
  function automatic rq_t gc(rq_t in); rq_t r = in; r.a++; r.q.push_back(4); cr.delete("x"); return r; endfunction
  function automatic rec_t gq(rec_t in); rec_t r = in; r.a++; rq.delete("x"); return r; endfunction
  function automatic string gs(string in); ss.delete("x"); return {in, "!"}; endfunction
  function automatic H gh(H in); hh.delete("x"); return in; endfunction
  function automatic int gi(int in); nq.delete("x"); return in + 1; endfunction
  function automatic int st(int in); nq["x"][0] = 50; return in; endfunction
  initial begin
    e.a = 40; e.s = "e";
    ra["x"][1].a = 1; ra["x"][3] = c ? g(ra["x"][1]) : e;
    $display("A %0d %0d %0d", ra.num(), ra["x"].num(), ra["x"][3].a);
    ra.delete(); ra["x"][1].a = 1; ra["x"][3] = !c ? g(ra["x"][1]) : e;
    $display("B %0d %0d %0d", ra.num(), ra["x"].num(), ra["x"][3].a);
    ra.delete(); ra["y"][1].a = 1; ra["x"][3] = !c ? g(ra["x"][1]) : e;
    $display("C %0d %0d %0d", ra.num(), ra["x"].num(), ra["x"][3].a);
    ra.delete(); ra["x"][1].a = 1; ra["x"][3] = c ? (c ? g(ra["x"][1]) : e) : e;
    $display("D %0d %0d %0d", ra.num(), ra["x"].num(), ra["x"][3].a);
    ra.delete(); ra["x"][1].a = 1; ra["x"][1].s = "k"; ra["x"][3] = x ? g(ra["x"][1]) : ra["x"][1];
    $display("E %0d %0d", ra.exists("x"), ra.exists("x") ? ra["x"].num() : -1);
    rq["x"].push_back(e); rq["x"][1] = c ? gq(rq["x"][0]) : e;
    $display("G %0d %0d", rq.exists("x"), rq["x"].size());
    ss["x"][1] = "k"; ss["x"][3] = c ? gs(ss["x"][1]) : "z";
    $display("H %0d %s", ss["x"].num(), ss["x"][3]);
    hh["x"][1] = new; hh["x"][1].v = 7; hh["x"][3] = gh(hh["x"][1]);
    $display("I %0d %0d", hh["x"].num(), hh["x"][3].v);
    nq["x"].push_back(5); nq["x"][1] = gi(nq["x"][0]);
    $display("J %0d %0d", nq.exists("x"), nq["x"].size());
    nq.delete(); nq["x"].push_back(5); v = st(nq["x"][0]);
    $display("K %0d %0d", v, nq["x"][0]);
    $finish;
  end
endmodule
