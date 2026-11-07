// Decision S13-D5: a fork that executes during static initialization stops the
// run, also when no static initializer was seen to reach it. Here the fork is
// in a function that a constraint calls while a static initializer's
// randomize call solves, a call the reachability checks do not follow. The
// fork has no parent process there, and the run stops naming the branch.
//
// IEEE 1800-2009 13.4.4 (SystemVerilog-1800-2009.txt L18993-18997): such a
// call "shall be allowed provided that the thread calling the function is
// created by an initial procedure, always procedure, or fork block from one of
// those procedures"; "Implementations shall issue an error either at compile
// time or run time when these provisions have not been met."
class item;
  static int n;
  rand int x;
  function int f();
    fork n = 7; join_none
    return 3;
  endfunction
  constraint c { x == f(); }
endclass
class holder;
  static item o = new;
  static int ok = mk();
  static function int mk(); return o.randomize(); endfunction
endclass
module tb;
  initial #1 $display("%0d %0d %0d", holder::ok, holder::o.x, item::n);
endmodule
