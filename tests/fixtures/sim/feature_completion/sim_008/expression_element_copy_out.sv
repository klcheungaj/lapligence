// SIM-008, decision EO-D3: an output or inout actual naming an element of a
// queue, dynamic or associative array (integral and string keys), also of a
// class property, bound to a function called inside an expression copies out
// when the call returns, before the rest of the expression reads on: packed,
// real and 4-state elements, static and automatic functions, a missing
// associative entry created by the copy-out, an index outside a queue that
// drops it, a copy-in after an input with an effect, and a call that `&&`
// skips. Two calls of one expression copy out in order.
//
// IEEE 1800-2009 13.5.2 (SystemVerilog-1800-2009.txt L19185-19187): "an
//   inout argument is copied twice: once from the actual into the argument
//   when the subroutine is called and once from the argument into the actual
//   when the subroutine returns."
class C;
  int a[4];
  int q[$];
  int da[];
endclass
module tb;
  C o;
  int q[$];
  int da[];
  int aa[int];
  int sa[string];
  real rq[$];
  logic [7:0] lq[$];
  int k, x;
  bit c;
  string s;
  function automatic int f(output int o, input int v);
    o = v;
    return v + 1;
  endfunction
  function automatic int g(inout int io);
    io = io * 2;
    return io;
  endfunction
  function int sg(inout int io);
    io = io + 100;
    return io;
  endfunction
  function automatic int r(output real o);
    o = 2.5;
    return 3;
  endfunction
  function automatic int l(inout logic [7:0] o);
    o = ~o;
    return 1;
  endfunction
  function automatic int bump();
    k = 2;
    return 0;
  endfunction
  function automatic int ab(input int a, inout int b);
    b = b + a;
    return b;
  endfunction
  initial begin
    q = '{1, 2, 3}; da = new[2]; aa[3] = 7; sa["k"] = 5;
    rq = '{0.5, 1.5}; lq = '{8'h0f};
    k = 1;
    x = f(q[k], 40) + 1;
    $display("a x=%0d q=%p", x, q);
    x = 2 + g(da[1]);
    $display("b x=%0d da=%p", x, da);
    x = g(aa[3]) + g(aa[3]);
    $display("c x=%0d aa3=%0d", x, aa[3]);
    x = sg(sa["k"]) * 2;
    $display("d x=%0d sa=%p", x, sa);
    x = r(rq[1]) + 1;
    $display("e x=%0d rq=%p", x, rq);
    x = l(lq[0]) + l(lq[0]) + l(lq[0]);
    $display("f x=%0d lq=%h", x, lq[0]);
    k = 1; x = ab(bump(), q[k]) + 0;
    $display("g x=%0d k=%0d q=%p", x, k, q);
    k = 0; c = 0; x = c && f(q[k], 9);
    $display("h x=%0d q=%p", x, q);
    k = 0; c = 1; x = c && f(q[k], 9);
    $display("i x=%0d q=%p", x, q);
    x = f(aa[11], 5) + aa.num();
    $display("j x=%0d aa3=%0d aa11=%0d num=%0d", x, aa[3], aa[11], aa.num());
    k = 5; x = f(q[k], 1) + 1;
    $display("k x=%0d q=%p", x, q);
    o = new;
    o.q = '{1, 2}; o.da = new[2];
    k = 1;
    x = f(o.a[k], 4) + 1;
    $display("l x=%0d a=%p", x, o.a);
    x = f(o.q[k], 5) + 1;
    $display("m x=%0d q=%p", x, o.q);
    x = f(o.da[k], 6) + 1;
    $display("n x=%0d da=%p", x, o.da);
    $finish(0);
  end
endmodule
