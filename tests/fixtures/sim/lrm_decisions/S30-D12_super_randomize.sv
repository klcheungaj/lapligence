// Decision S30-D12: super.randomize() randomizes the whole object.
//
// IEEE 1800-2009 18.6.1 (SystemVerilog-1800-2009.txt L30397-30402):
//   "virtual function int randomize();"
//   "The randomize() method is a virtual function that generates random values
//   for all the active random variables in the object, subject to the active
//   constraints."
//
// The clause gives super.randomize() no meaning of its own. llg follows the
// industry-standard behaviour: the call acts like this.randomize(), on every
// random variable of the object's dynamic class, with the constraints in
// effect for that class, including derived constraints and overrides, and
// runs the most derived pre_randomize() and post_randomize().
class base_c;
  rand bit [3:0] x;
  constraint c0 { x < 4; }
  constraint over { x != 1; }
  function void post_randomize();
    $display("base post");
  endfunction
endclass

class mid_c extends base_c;
  rand bit [3:0] y;
  constraint c1 { y == x + 1; }
  constraint over { x != 2; }
  function int via_super();
    return super.randomize();
  endfunction
  function int via_super_with();
    return super.randomize() with { x == 0; };
  endfunction
endclass

class top_c extends mid_c;
  rand bit [3:0] z;
  constraint c2 { z == y + 1; }
  function void post_randomize();
    $display("top post");
  endfunction
  function int via_super_list();
    return super.randomize(null);
  endfunction
endclass

module tb;
  initial begin
    static top_c t = new;
    int ok;
    int seen[4];
    repeat (40) begin
      t.x = 15;
      t.y = 15;
      t.z = 15;
      ok = t.via_super();
      if (ok != 1 || t.y != t.x + 1 || t.z != t.y + 1)
        $display("bad %0d %0d %0d", t.x, t.y, t.z);
      seen[t.x]++;
    end
    // x == 1 is excluded by the base's body of `over`, which mid_c replaces,
    // so 1 is legal and 2 is not.
    $display("x0=%0d x1=%0d x2=%0d x3=%0d", seen[0] > 0, seen[1] > 0, seen[2] > 0, seen[3] > 0);
    ok = t.via_super_with();
    $display("with %0d x=%0d y=%0d z=%0d", ok, t.x, t.y, t.z);
    t.x = 3;
    t.y = 4;
    t.z = 5;
    ok = t.via_super_list();
    $display("list %0d x=%0d y=%0d z=%0d", ok, t.x, t.y, t.z);
    $finish(0);
  end
endmodule
