// ADV-019 family 3: the VPI application constraints.c walks the constraints
// of this design's classes (IEEE 1800-2009 37.30, 37.34): their qualifiers
// and state, from the class definition and from an object, and the tree of
// constraint items. See readme.md for the expected output.
module tb;
  class Packet;
    rand bit [3:0] a;
    rand int d;
    rand bit [7:0] len;
    constraint c_small { a > 2; }
    static constraint c_shared { len != 0; }
    constraint c_mix {
      d dist { 0 := 1, [1:3] :/ 2, 7 };
      solve a before d;
      (d < 10) -> { a == 3; len < 8; }
      if (d > 2) a < 5; else { a > 1; }
      if (len > 4) d inside {[0:20], 30};
    }
  endclass

  Packet p;
  int ok;

  initial begin
    p = new;
    ok = p.randomize();
    p.c_small.constraint_mode(0);
    $constraints();
    // A static constraint's mode is shared by every object (18.9).
    p.c_shared.constraint_mode(0);
    p.c_small.constraint_mode(1);
    $modes();
    $display("ok=%0d", ok);
  end
endmodule
