// The same member of a static class property named from every scope: inside
// the class, through `this` and `super`, a derived class, a parameterized
// class, a package, an object in a container, a class property and a virtual
// interface (readme: "Scopes").
`timescale 1ns / 1ns
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;
typedef struct {
  int a;
  string s;
} rec_t;
package pk;
  typedef struct packed {
    logic [3:0] hi, lo;
  } pair_t;
  class st;
    static pair_t pr = 8'h12;
    static int fa[2] = '{3, 4};
  endclass
endpackage
class st;
  static pair_t pr;
  static rec_t rec;
  static rec_t ra[2];
  int own = 1;
  function void plain();
    pr.hi = 1;
    rec.a = 10;
    ra[0].s = "plain";
  endfunction
  function void itself();
    this.pr.lo = 2;
    this.rec.s = "this";
    this.ra[1].a = 20;
  endfunction
  static function int total();
    return pr.hi + pr.lo + rec.a + ra[1].a;
  endfunction
  function void show(string from);
    $display("%s: %h %0d %s %s %0d, %0d", from, pr, rec.a, rec.s, ra[0].s, ra[1].a, total());
  endfunction
endclass
class derived extends st;
  function void both();
    super.pr.hi = 3;
    super.rec.a++;
    pr.lo = 4;
    ra[1].a += 1;
  endfunction
endclass
class par #(
    int n = 1,
    type t = int
);
  static pair_t pr = n;
  static t v;
  static rec_t ra[2];
endclass
class holder;
  st kept = new;
  static st shared = new;
endclass
interface bus;
  st o = new;
endinterface
module tb;
  st o = new, objects[2], q[$], none;
  derived d = new;
  holder h = new;
  par #(5, rec_t) five = new;
  bus b ();
  virtual bus v = b;
  initial begin
    o.plain();
    o.itself();
    o.show("methods");
    d.both();
    d.show("derived");
    derived::pr.hi = 5;
    d.rec.s = "object";
    o.show("names");
    objects[1] = o;
    q.push_back(d);
    objects[1].pr.lo = 6;
    q[0].rec.a = 7;
    h.kept.ra[0].s = "kept";
    holder::shared.ra[1].a = 8;
    h.shared.pr.hi = 9;
    o.show("paths");
    // A static property needs no object (8.9).
    none.pr.lo = 10;
    none.rec.s = "none";
    o.show("no object");
    b.o.pr.hi = 11;
    v.o.rec.a = 12;
    v.o.ra[1].a <= 13;
    #1 $display("interface %0d %h %0d", v.o.rec.a, v.o.pr.hi, b.o.ra[1].a);
    o.show("interface");
    // Each specialization has its own properties (8.25).
    par#(5, rec_t)::pr.hi = 6;
    five.v.a = 14;
    par#(5, rec_t)::v.s = "five";
    par#(7, int)::ra[1].a = 15;
    par#(7, int)::v = 16;
    $display("five %h %0d %s %0d", par#(5, rec_t)::pr, par#(5, rec_t)::v.a, five.v.s,
             five.ra[1].a);
    $display("seven %h %0d %0d", par#(7, int)::pr, par#(7, int)::ra[1].a, par#(7, int)::v);
    pk::st::pr.hi = 7;
    pk::st::fa[1]++;
    $display("package %h %0d %h", pk::st::pr, pk::st::fa[1], st::pr);
  end
endmodule
