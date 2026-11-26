`timescale 1ns/1ns
module tb;
  event a, b, c;
  bit s;
  event ea[2];
  event q[$];
  class holder;
    event e;
  endclass
  holder o = new;

  function event pick(bit k);
    return k ? a : b;
  endfunction
  function automatic event of(event p, event r, bit k);
    return k ? p : r;
  endfunction
  task automatic waits(input event e, input int k);
    @(e);
    $display("w%0d %0t", k, $time);
  endtask
  task automatic trigger(input event e);
    -> e;
  endtask

  initial begin
    s = 1;
    c = s ? a : b;
    $display("%b %b", c == a, c == b);
    fork begin @c; $display("via c %0t", $time); end join_none
    #1 -> a;
    #1 s = 0;
    c = s ? a : b;
    $display("%b", c == b);
    c = pick(1);
    $display("%b %b", c == a, of(a, b, 0) == b);
    o.e = s ? a : b;
    $display("%b", o.e == b);
    c = s ? o.e : ea[1];
    $display("%b", c == ea[1]);
    ea = '{s ? a : b, a};
    $display("%b", ea[0] == b);
    q.push_back(s ? a : b);
    $display("%b", q[0] == b);
    $display("%b %b", (s ? a : b) == b, a != (s ? b : a));
    if (s ? a : b) $display("set");
    fork
      waits(s ? a : b, 1);
      waits(!s ? a : b, 2);
      begin @(s ? a : b); $display("at %0t", $time); end
    join_none
    #1 trigger(s ? b : a);
    #1 trigger(s ? a : b);
    #1 $finish;
  end
endmodule
