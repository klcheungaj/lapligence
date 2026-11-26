// Functions with nonblocking assignments of their own as operands of a
// nonblocking assignment with an event control (readme: "Event controls").
`timescale 1ns / 1ns
module tb;
  typedef struct {
    int a;
    string s;
  } rec_t;
  typedef struct {
    rec_t in;
    real few[2];
    string names[2];
  } big_t;
  rec_t one, ra[2];
  big_t nn, na[2];
  int ia[3], ib[3], v, w, x;
  event ev;
  function int f(int k);
    v <= k;
    return k + 1;
  endfunction
  function string g(int k);
    w <= k;
    return $sformatf("s%0d", k);
  endfunction
  function int ix(int k);
    x <= 100 + k;
    return k;
  endfunction
  function rec_t whole(int k);
    v <= k;
    whole.a = k;
    whole.s = g(k);
  endfunction
  function int part(int k);
    ra[0].a <= k;
    one.s <= "part";
    return k;
  endfunction
  always #10 ->ev;
  // The operand's updates are made in this time slot, the write's after the
  // event.
  task show(string step);
    #1 $display("%0t %s: v %0d w %0d x %0d", $time, step, v, w, x);
  endtask
  initial begin
    one.a <= @(ev) f(1);
    one.s <= @(ev) g(2);
    show("members");
    ra[ix(1)].s <= @(ev) g(3);
    ra[1].a <= repeat (f(0)) @(ev) f(4);
    show("element members");
    one <= @(ev) whole(5);
    show("record");
    ra[ix(0)] <= @(ev) '{f(6), g(7)};
    show("element");
    na[ix(1)].in <= @(ev) whole(8);
    show("element record");
    nn.names[ix(1)] <= @(ev) g(12);
    na[0].few <= @(ev) '{f(13), 2.5};
    show("arrays");
    $display("%0t %0d '%s' '%s' '%s'", $time, one.a, one.s, ra[1].s, nn.names[1]);
    #10 $display("%0t %0d %s %0d %s %0d %s", $time, one.a, one.s, ra[0].a, ra[0].s, ra[1].a, ra[1].s);
    $display("%0d %s %s %0.1f", na[1].in.a, na[1].in.s, nn.names[1], na[0].few[0]);
    ia <= @(ev) '{f(14), 2, part(3)};
    {ib[0], ib[1]} <= @(ev) {f(15), f(16)};
    ib[ix(2)] <= @(ev) f(17);
    show("packed");
    $display("%0t %0d %s %p %p", $time, ra[0].a, one.s, ia, ib);
    #10 $display("%0t %p %p", $time, ia, ib);
    $finish;
  end
endmodule
