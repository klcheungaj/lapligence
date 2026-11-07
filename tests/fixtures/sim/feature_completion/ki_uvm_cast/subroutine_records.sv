typedef enum {A, B} kind_e;
typedef struct { kind_e kind; bit [63:0] addr; int n; bit [7:0] be; } op_t;
typedef struct { int n; byte b; } two_t;
class C;
  int a;
  function new(int x = 0);
    a = x;
  endfunction
endclass
class ada;
  virtual function C r2b(const ref op_t rw);
    C c = new(rw.n);
    return c;
  endfunction
  virtual function void b2r(C c, ref op_t rw);
    rw.n = c.a + 1;
  endfunction
  function void inc(ref two_t t);
    t.n++;
    t.b = 8'h7f;
  endfunction
endclass
class m;
  ada ad = new;
  op_t acc[$];
  two_t byname[string];
  task t1();
    op_t rw;
    C c;
    rw.n = 4;
    c = ad.r2b(rw);
    $display("%0d", c.a);
    ad.b2r(c, rw);
    $display("%0d %0d", rw.n, rw.kind);
  endtask
  task t2();
    op_t local_acc[$];
    op_t e;
    e.n = 7;
    local_acc.push_back(e);
    e.n = 9;
    local_acc.push_back(e);
    foreach (local_acc[i]) begin
      op_t r = local_acc[i];
      C c;
      c = ad.r2b(r);
      $display("%0d", c.a);
    end
  endtask
  function void t3();
    two_t e;
    e.n = 1;
    e.b = 2;
    byname["k"] = e;
    acc.push_back('{B, 64'h10, 3, 8'h1});
    begin
      op_t r = acc[0];
      two_t t = byname["k"];
      ad.inc(t);
      $display("%0d %h %0d %h %0d %h", r.kind, r.addr, r.n, r.be, t.n, t.b);
    end
  endfunction
endclass
module tb;
  m o = new;
  op_t acc[$];
  two_t t;
  initial begin
    o.t1();
    o.t2();
    o.t3();
    acc.push_back('{A, 64'h20, 5, 8'h2});
    foreach (acc[i]) begin
      automatic op_t r = acc[i];
      $display("%0d %h", r.n, r.be);
    end
    $finish;
  end
endmodule
