typedef struct { int a; string s; real r; } rec_t;
class H;
  rec_t r;
  virtual function rec_t get();
    int t = tb.del();
    r.a = 10;
    return r;
  endfunction
endclass
class H2 extends H;
  virtual function rec_t get();
    int t = tb.del();
    r.a = 11;
    return r;
  endfunction
endclass
class C;
  static function rec_t smk();
    int t = tb.del();
    rec_t r;
    r.a = 9;
    return r;
  endfunction
endclass
module tb;
  rec_t ra[string][int];
  bit c = 1;
  logic x = 1'bx;
  int n;
  rec_t e, e2;
  rec_t fa[2];
  rec_t qr[$];
  H h = new;
  H2 hv = new;
  H v = hv;
  function automatic int del(); ra.delete("x"); return 1; endfunction
  function automatic string dels(); ra.delete("x"); return "z"; endfunction
  function automatic rec_t mk(int value); rec_t r; r.a = value; ra.delete("x"); return r; endfunction
  function automatic rec_t mk2(rec_t in); rec_t r = in; r.a++; ra.delete("x"); return r; endfunction
  function automatic rec_t mko(output int o); rec_t r; r.a = 6; o = 1; ra.delete("x"); return r; endfunction
  function automatic H mkh(); H t = new; t.r.a = 12; ra.delete("x"); return t; endfunction
  task automatic reset();
    ra.delete();
    ra["x"][1].a = 1;
    ra["y"][1].a = 70;
    qr.delete();
    qr.push_back(fa[0]);
    qr.push_back(fa[1]);
  endtask
  task automatic show(string name);
    $display("%s %0d %0d %0d", name, ra.exists("x"), ra.exists("x") ? ra["x"].num() : -1,
             ra.exists("x") && ra["x"].exists(3) ? ra["x"][3].a : -1);
  endtask
  initial begin
    e.a = 40; e2.a = 41; fa[0].a = 50; fa[1].a = 51; h.r.a = 60;
    reset(); ra["x"][3] = mk(4); show("call");
    reset(); ra["x"][3] = c ? mk(4) : e; show("cond");
    reset(); ra["x"][3] = x ? mk(4) : e; show("merge");
    reset(); ra["x"][3] = del() ? e : e2; show("predicate");
    reset(); ra["x"][3] = c ? (c ? mk(4) : e) : e2; show("nested");
    reset(); ra["x"][3] = '{a: del(), s: "p", r: 0.5}; show("pattern");
    reset(); ra["x"][3] = ra["y"][del()]; show("element");
    reset(); ra["x"][3] = fa[del() % 2]; show("fixed");
    reset(); ra["x"][3] = qr[del() % 2]; show("queue");
    reset(); ra["x"][3] = h.get(); show("method");
    reset(); ra["x"][3] = v.get(); show("virtual");
    reset(); ra["x"][3] = mkh().r; show("property");
    reset(); ra["x"][3] = mkh().get(); show("handle method");
    reset(); ra["x"][3] = qr.pop_front(); show("pop");
    reset(); ra["x"][3] = rec_t'(mk(4)); show("cast");
    reset(); ra["x"][3] = c ? ra["x"][1] : mk(4); show("element arm");
    reset(); ra["x"][3] = mk2(ra["x"][1]); show("element actual");
    reset(); ra["x"][3] = C::smk(); show("static");
    reset(); ra["x"][3] = mko(n); show("output");
    reset(); ra["x"][3].r = del(); show("real conversion");
    reset(); ra["x"][3].r = c ? 1.5 : real'(del()); show("real conditional");
    reset(); ra["x"][3].s = dels(); show("string");
    reset(); ra["x"][3].s = c ? dels() : "q"; show("string conditional");
    reset(); ra["x"][3].s = $sformatf("%0d", del()); show("string format");
    $finish;
  end
endmodule
