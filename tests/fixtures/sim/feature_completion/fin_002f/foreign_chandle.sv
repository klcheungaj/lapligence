// Chandles whose C object is gone (6.14, 35.5.1.5): llg passes the value on and
// never looks at what it refers to.
`timescale 1ns/1ns
module tb;
  import "DPI-C" function chandle c_new();
  import "DPI-C" function void c_free(input chandle h);
  import "DPI-C" function int c_state(input chandle h);
  import "DPI-C" function chandle c_same(input chandle h);
  import "DPI-C" function void c_swap(inout chandle a, inout chandle b);
  import "DPI-C" function chandle c_wild();
  class holder;
    chandle h;
    chandle many[$];
  endclass
  chandle a, b, none;
  holder o;
  mailbox #(chandle) mb = new;
  holder cast_to;

  initial begin
    $display("null %0d %0d", c_state(none), none == null);
    a = c_new();
    b = a;
    o = new;
    o.h = a;
    o.many = '{a, null, c_new()};
    mb.put(a);
    c_free(a);
    $display("freed %0d %0d %0d", c_state(a), c_state(b), c_state(o.h));
    $display("same %0d %0d", a == b, c_same(a) == a);
    mb.get(b);
    $display("mailbox %0d %0d", b == a, c_state(b));
    $display("queue %0d %0d %0d", c_state(o.many[0]), c_state(o.many[1]), c_state(o.many[2]));
    b = c_new();
    c_swap(a, b);
    $display("swapped %0d %0d", c_state(a), c_state(b));
    none = c_wild();
    $display("wild %0d %0d", none != null, c_same(none) == none);
    fork
      begin
        automatic chandle kept = none;
        #1 $display("kept %0d", kept == none);
      end
    join
    $display("cast %0d %0d", $cast(cast_to, none), cast_to == null);
    $display("cast %0d %0d", $cast(cast_to, a), cast_to == null);
    if (none) $display("not null");
    o = null;
    $finish;
  end
endmodule
