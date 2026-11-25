`timescale 1ns/1ns
module tb;
  class C;
    rand bit [7:0] a;
    rand bit [7:0] b;
    int hi = 10;
    constraint c { a < hi; b > 3; }
    function int attempt();
      return this.randomize() + 4;
    endfunction
    function bit inline();
      return randomize() with { a == 2; };
    endfunction
    function bit some();
      return this.randomize(a);
    endfunction
  endclass
  class D;
    rand bit [3:0] a;
    bit [3:0] value = 5;
    constraint c { a == value; }
  endclass

  C o = new;
  D d = new;
  int x;
  bit z;
  logic [3:0] l;
  bit [7:0] loc;
  string s;
  int arr[8], q[3], dyn[];

  function automatic int twice(int k);
    return k * 2;
  endfunction
  function automatic int both(int k, int m);
    return k * 10 + m;
  endfunction
  function automatic bit plain(C c);
    return c.randomize();
  endfunction
  function automatic bit five(C c);
    return c.randomize() with { a == 5; };
  endfunction
  task t(int k);
    $display("task %0d", k);
  endtask

  initial begin
    // operators
    x = o.randomize() + 1;
    $display("%0d %b", x, o.a < 10);
    x = 1 + o.randomize() * 2;
    $display("%0d", x);
    x = o.randomize() with { a == 3; } + 4;
    $display("%0d %0d", x, o.a);
    x = (o.randomize() with { a == 2; }) + (o.randomize() with { a == 4; }) * 4;
    $display("%0d %0d", x, o.a);
    x = -o.randomize();
    $display("%0d", x);
    x += o.randomize();
    $display("%0d", x);
    l = {o.randomize(), 1'b0};
    $display("%b", l[1:0]);
    // an operand before the call reads the value the call changes
    x = d.a + d.randomize();
    $display("%0d %0d", x, d.a);
    d.value = 7;
    x = both(d.randomize(), d.a);
    $display("%0d", x);
    d.value = 8;
    x = both(d.a, d.randomize());
    $display("%0d %0d", x, d.a);
    // conditions
    if (!o.randomize() with { a == 7; }) $display("bad");
    else $display("if %0d", o.a);
    if (o.randomize() with { a == 1; } && o.a == 1) $display("and");
    z = 0;
    if (z && o.randomize() with { a == 9; }) $display("bad");
    $display("skipped %0d", o.a);
    z = o.randomize() ? 1 : 0;
    $display("%b", z);
    x = o.randomize() ? (o.randomize() with { a == 6; } ? 10 : 20) : 30;
    $display("%0d %0d", x, o.a);
    while (!o.randomize()) $display("never");
    do x++; while (o.randomize() && x < 14);
    $display("%0d", x);
    for (int k = 0; k < o.randomize() + 1; k++) $display("for %0d", k);
    repeat (o.randomize() + 1) $display("repeat");
    case (o.randomize() + 1) 2: $display("two"); default: $display("bad"); endcase
    case (1) o.randomize(): $display("item"); default: $display("bad"); endcase
    randcase o.randomize(): $display("randcase"); 0: $display("bad"); endcase
    if (1 inside {o.randomize()}) $display("inside");
    assert (o.randomize() with { b == 9; } && o.b == 9) $display("assert");
    else $display("bad");
    // arguments, returns, selects, patterns
    $display("%0d %0d", o.randomize(), twice(o.randomize()));
    $display("%0d", twice(o.randomize() with { a == 9; }) + o.a);
    $display("%b %b %0d", plain(o), five(o), o.a);
    $display("%0d %b %b", o.attempt(), o.inline(), o.some());
    s = $sformatf("%0d", o.randomize());
    $display("%s", s);
    t(o.randomize());
    arr[o.randomize()] = 3;
    $display("%0d %0d", arr[1], arr[o.randomize()]);
    q = '{o.randomize(), 0, 2};
    $display("%p", q);
    dyn = new[o.randomize() + 1];
    $display("%0d", dyn.size());
    foreach (q[i]) if (q[i] == o.randomize()) $display("match %0d", i);
    #(o.randomize()) $display("%0t", $time);
    x <= o.randomize() + 1;
    #1 $display("%0d", x);
    fork
      x = o.randomize() + 5;
    join
    $display("%0d", x);
    x = std::randomize(loc) with { loc == 4; } + loc;
    $display("%0d", x);
    // a failure
    o.hi = 0;
    x = o.randomize() + 5;
    $display("%0d", x);
    $finish(0);
  end
endmodule
