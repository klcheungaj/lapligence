// Implicit sensitivity follows a whole unpacked member, or a slice of one,
// read through a virtual-interface variable to the instance it binds
// (decision S99-D7), also as the actual of a called function.
interface ifc;
  int a[4];
endinterface

module tb;
  ifc i();
  ifc j();
  virtual ifc v = i, w = j;
  int y, n, t, f;
  int lq[4];
  function automatic int ends(input int x[4]); return x[0] + x[3]; endfunction
  always_comb begin v.a = '{n, n + 1, n + 2, n + 3}; y = v.a.sum(); end
  always_comb lq[0:1] = w.a[1:2];
  always @* t = v.a[1] + w.a.sum();
  always_comb f = ends(w.a);
  always @* if (v.a == w.a) $display("same at %0t", $time);
  initial begin
    #1 $display("%0d %p %0d", y, i.a, t);
    n = 5;
    #1 $display("%0d %p %0d", y, i.a, t);
    w.a[2] = 7;
    #1 $display("%p %0d %0d", lq, t, f);
    w.a = i.a;
    #1 $display("%p %0d %0d", lq, t, f);
    v = j; n = 9;
    #1 $display("%p %p %0d %0d %0d", i.a, j.a, y, t, f);
    $finish(0);
  end
endmodule
