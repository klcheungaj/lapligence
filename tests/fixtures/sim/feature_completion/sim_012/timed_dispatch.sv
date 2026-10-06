// SIM-012 A01: timed interface tasks run through virtual interfaces held in
// variables, class properties, containers and records; each call stays on the
// instance its receiver named at the call (SV 25.9, 9.3.2).
interface ifc;
  int count;
  task automatic bump(int n);
    #n count += n;
  endtask
  function int get(); return count; endfunction
endinterface

class holder;
  virtual ifc v;
  virtual ifc pool[2];
  function new(virtual ifc x); v = x; endfunction
  task run(int n); v.bump(n); endtask
endclass

typedef struct { virtual ifc v; string tag; } rec_t;

module tb;
  ifc a(), b();
  virtual ifc v;
  virtual ifc m[string];
  virtual ifc q[$];
  holder h;
  rec_t r;

  task automatic use_rec(input rec_t x, input int n);
    x.v.bump(n);
    $display("%s count=%0d t=%0d", x.tag, x.v.get(), $time);
  endtask

  initial begin
    v = a;
    h = new(b);
    fork
      v.bump(2);
      h.run(3);
      begin #1 v = b; end
    join
    $display("a=%0d b=%0d t=%0d", a.count, b.count, $time);
    v.bump(1);
    $display("a=%0d b=%0d get=%0d", a.count, b.count, v.get());
    m["a"] = a;
    q.push_back(b);
    h.pool[1] = a;
    fork
      m["a"].bump(4);
      q[0].bump(5);
      h.pool[1].bump(6);
    join
    $display("a=%0d b=%0d t=%0d", a.count, b.count, $time);
    r.v = b;
    r.tag = "rec";
    use_rec(r, 2);
    h.v.count = 20;
    h.pool[1].count = 30;
    $display("a=%0d b=%0d", a.count, b.count);
    $finish;
  end
endmodule
