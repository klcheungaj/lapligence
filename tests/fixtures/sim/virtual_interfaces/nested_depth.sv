// Nested interface instances two and three levels deep through a virtual
// interface (SV 25.9): selection as a value, member reads and writes and
// method calls through the whole path, and a parent handle rebound between
// two instances.
`timescale 1ns/1ns
interface leaf_if;
  logic [7:0] v;
  int hits;
  function void bump();
    hits++;
  endfunction
endinterface
interface mid_if;
  logic [7:0] m;
  leaf_if l();
endinterface
interface outer_if;
  logic [7:0] m;
  mid_if a();
  mid_if b();
endinterface
class user;
  virtual outer_if o;
  virtual mid_if mv;
  virtual leaf_if lv;
  function void go(logic [7:0] base);
    lv = o.a.l;
    lv.v = base + 1;
    mv = o.b;
    lv = mv.l;
    lv.v = base + 2;
    o.a.l.v = o.a.l.v + 8'h10;
    o.b.m = base + 3;
    o.m = base + 4;
    o.a.l.bump();
    lv.bump();
    mv.l.bump();
  endfunction
endclass
module tb;
  outer_if o1();
  outer_if o2();
  initial begin
    user u = new;
    u.o = o2;
    u.go(8'h20);
    #1;
    $display("o1 %h %h %h %h", o1.a.l.v, o1.b.l.v, o1.b.m, o1.m);
    $display("o2 %h %h %h %h", o2.a.l.v, o2.b.l.v, o2.b.m, o2.m);
    $display("hits %0d %0d %0d", o2.a.l.hits, o2.b.l.hits, o1.a.l.hits);
    u.o = o1;
    u.go(8'h40);
    #1;
    $display("o1 %h %h %h %h", o1.a.l.v, o1.b.l.v, o1.b.m, o1.m);
    $display("eq %0d %0d %0d", u.lv == o1.b.l, u.lv == o2.b.l, u.mv == o1.b);
    $finish;
  end
endmodule
