// Class `$cast` destinations of every storage kind, successful and failing
// (SV 6.24.2).
class base; int a; function new(int x=0); a=x; endfunction endclass
class der extends base; function new(int x=0); super.new(x); endfunction endclass
class o2 extends base; function new(int x=0); super.new(x); endfunction endclass
class box; der h; static der s; der q[$]; endclass
typedef struct { der h; int n; } rec_t;
module tb;
  function automatic base mk(int k); der d; o2 o; if (k == 1) begin d = new(7); return d; end o = new(8); return o; endfunction
  function automatic bit put(ref der r, input int k); return $cast(r, mk(k)); endfunction
  der fa[2]; der v; der q[$]; der m[int]; rec_t r;
  initial begin
    box b = new;
    $display("var %0d %0d", $cast(v, mk(1)), v.a);
    $display("prop %0d %0d", $cast(b.h, mk(1)), b.h.a);
    $display("static %0d %0d", $cast(box::s, mk(1)), box::s.a);
    $display("elem %0d %0d", $cast(fa[1], mk(1)), fa[1].a);
    $display("queue %0d %0d", $cast(q[0], mk(1)), q.size());
    $display("assoc %0d %0d", $cast(m[3], mk(1)), m[3].a);
    $display("member %0d %0d", $cast(r.h, mk(1)), r.h.a);
    $display("propq %0d %0d", $cast(b.q[0], mk(1)), b.q.size());
    $display("ref %0d %0d", put(fa[0], 1), fa[0].a);
    $display("fail var %0d %0d", $cast(v, mk(2)), v.a);
    $display("fail prop %0d %0d", $cast(b.h, mk(2)), b.h.a);
    $display("fail ref %0d %0d", put(fa[0], 2), fa[0].a);
    $display("null %0d %0d", $cast(v, null), v == null);
    $finish;
  end
endmodule
