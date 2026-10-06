// SIM-011: `new h` and `new this` make a shallow copy (SV 8.11): a new object
// whose properties are copied from the source without running constructors
// or property initializers; a handle property names the same object, while
// strings, queues and records are copied values.
module tb;
  int ctor_runs, init_runs;
  function int note(); init_runs++; return 7; endfunction
  typedef struct { string s; int n; } rec_t;

  class item;
    int v;
    function new(int x); v = x; endfunction
  endclass

  class pkt;
    int a;
    logic [3:0] b = 4'bx1z0;
    real r;
    string s;
    item h;
    int q[$];
    rec_t rec;
    int k = note();
    function new();
      ctor_runs++;
      a = 1; r = 1.5; s = "orig"; h = new(3); q = '{1, 2}; rec = '{"rs", 9};
    endfunction
    function pkt dup(); dup = new this; endfunction
  endclass

  class ext extends pkt;
    string tag = "ext";
  endclass

  initial begin
    pkt p, c, d;
    ext e, f;
    p = new;
    c = new p;
    p.a = 10; p.r = 2.5; p.s = "changed"; p.q.push_back(3); p.rec.s = "pr"; p.h.v = 4;
    $display("c a=%0d b=%b r=%0.1f s=%s q=%0d:%0d rec=%s/%0d k=%0d", c.a, c.b, c.r, c.s,
             c.q.size(), c.q[1], c.rec.s, c.rec.n, c.k);
    $display("c h.v=%0d same=%0d", c.h.v, c.h == p.h);
    $display("runs ctor=%0d init=%0d", ctor_runs, init_runs);
    d = p.dup();
    $display("d a=%0d s=%s q=%0d", d.a, d.s, d.q.size());
    e = new;
    e.tag = "mine"; e.a = 5;
    f = new e;
    $display("f tag=%s a=%0d ctor=%0d", f.tag, f.a, ctor_runs);
    $finish;
  end
endmodule
