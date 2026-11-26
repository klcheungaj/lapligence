// Timed blocking writes of unpacked members through a virtual interface
// locate their target when the control has passed: an element of an array of
// handles is selected then, with the index then, for elements of fixed, real,
// string, record and queue members and for a whole member, after a delay, an
// event control and a repeated event control. The right-hand side is
// evaluated at once.
// IEEE 1800-2009 10.4.1 L13024-13025: "If variable_lvalue requires an
// evaluation, it shall be evaluated at the time specified by the
// intra-assignment timing control."
interface ifc;
  int a[4];
  int q[$];
  string s[2];
  typedef struct { int x; byte y; } rec_t;
  rec_t rs[2];
  real r[2];
endinterface
module tb;
  ifc i();
  ifc j();
  virtual ifc va[2];
  virtual ifc v;
  int k;
  event e;
  initial begin
    va[0] = i; va[1] = j; k = 0;
    i.q = '{0, 0}; j.q = '{0, 0};
    fork
      va[k].a[2] = #1 6;
      va[k].a = #2 '{1, 2, 3, va[k].a[2]};
      va[k].q[1] = #1 7;
      va[k].s[1] = #1 "x";
      va[k].rs[0] = #1 '{x: 3, y: 4};
      va[k].r[1] = #1 2.5;
      va[k].a[0] = @(e) 9;
      va[k].q[0] = repeat (2) @(e) 8;
      #0 k = 1;
      begin #3 -> e; #1 -> e; end
    join
    $display("%p %p", i.a, j.a);
    $display("%p %p %p %p", i.q, j.q, i.s, j.s);
    $display("%p %p %p %p", i.rs, j.rs, i.r, j.r);
    v = i;
    fork
      v.a[1] = #1 v.a[0];
      #0 v = j;
    join
    $display("%p %p", i.a, j.a);
    $finish;
  end
endmodule
