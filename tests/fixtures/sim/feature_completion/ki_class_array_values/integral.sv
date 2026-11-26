// Whole uses of fixed arrays of integral elements that are class
// properties: reductions, locator and ordering methods, copies into
// automatic arrays, returns, arguments and equality, inside the class and
// through a handle.
module tb;
  typedef int a4_t[4];

  class c;
    int la[4];
    byte lb[2:0];

    function int total();
      return la.sum();
    endfunction

    function a4_t get();
      return la;
    endfunction

    function int copy_third();
      int t[4];
      t = la;
      return t[2];
    endfunction

    function bit same(int t[4]);
      return la == t;
    endfunction

    function void order();
      la.sort();
      lb.rsort();
    endfunction
  endclass

  function automatic int third(c h);
    int t[4];
    t = h.la;
    return t[2];
  endfunction

  c o;
  int t[4];
  int q[$];

  initial begin
    o = new;
    o.la = '{3, 1, 4, 2};
    o.lb = '{5, 9, 7};
    $display("%0d %0d %0d %0d", o.total(), o.la.sum(), o.la.product(), o.lb.xor());
    $display("%0d %0d %0d", o.la.sum() with (item * 2), o.la.and(), o.la.or());
    q = o.la.find with (item > 2);
    $display("%p", q);
    t = o.get();
    $display("%p %0d %0d", t, o.copy_third(), third(o));
    $display("%b %b %b %b", o.la == t, t != o.la, o.same(t), o.la === t);
    t[0] = 0;
    $display("%b %b %b", o.la == t, t != o.la, o.same(t));
    o.order();
    $display("%p %p", o.la, o.lb);
    o.la.reverse();
    o.lb.sort();
    $display("%p %p", o.la, o.lb);
    $finish;
  end
endmodule
