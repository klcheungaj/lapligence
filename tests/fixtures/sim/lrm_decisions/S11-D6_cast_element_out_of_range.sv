// Decision S11-D6: a `$cast` that binds its destination (S11-D5) stops at an
// index with no element: a run-time error ends the simulation before the
// source runs, in either form, and the final procedures run; the exit status
// is 1. A queue index one past the end is the element the cast creates.
//
// IEEE 1800-2009 6.24.2 (SystemVerilog-1800-2009.txt L7433-7437): "When called
// as a function, $cast attempts to assign the source expression to the
// destination variable and returns 1 if the cast is legal." The text does not
// describe a destination element that does not exist; llg follows
// industry-standard behaviour.
class base;
endclass
class der extends base;
endclass
module tb;
  function automatic int f(string s, int r);
    $display("f(%s)", s);
    return r;
  endfunction
  function automatic base mk(string s);
    der d = new;
    $display("mk(%s)", s);
    return d;
  endfunction
  der q[$];
  initial begin
    $display("%0d %0d", $cast(q[0], mk("first")), q.size());
    $display("%0d", $cast(q[f("sel", 5)], mk("second")));
    $display("unreachable");
  end
  final $display("final");
endmodule
