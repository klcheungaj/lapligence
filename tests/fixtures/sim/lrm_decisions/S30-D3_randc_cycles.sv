// IEEE 1800-2009 18.4.2 L29442-29443: "The permutation sequence for any given
// randc variable is recomputed whenever the constraints change on that
// variable or when none of the remaining values in the permutation can
// satisfy the constraints." L29450-29451: "A set of constraints that includes
// both rand and randc variables shall be solved so that the randc variables
// are solved first, and this can sometimes cause randomize() to fail." The
// text does not define a change of constraints.
// Decision S30-D3 (llg reading): each object keeps one cycle per randc
// variable. A randc value is drawn uniformly from the unused values that
// still have a legal completion, so randomize() fails only when the
// constraints are infeasible. A new cycle starts when the constraint
// structure on the variable changes (constraint blocks switched on or off, a
// different inline constraint) or when no unused value is legal; a changed
// state value is not a change of constraints. A failed or inactive call does
// not advance the cycle.
class pair_c;
  randc bit [1:0] r;
  rand bit [1:0] y;
  constraint k { y > r; }
endclass

class state_c;
  randc bit [1:0] r;
  bit [1:0] skip;
  constraint k { r != skip; }
endclass

module tb;
  pair_c pc;
  state_c sc;
  int fails;
  bit [3:0] seen;

  initial begin
    // r == 3 has no legal y: r cycles over 0..2 and no call fails.
    pc = new;
    for (int i = 0; i < 3; i++) begin
      if (!pc.randomize()) fails = fails + 1;
      seen[pc.r] = 1'b1;
    end
    $display("fails=%0d cycle=%b", fails, seen);
    // Three draws with skip == 0 use 1, 2 and 3. With skip == 3 the cycle
    // continues: its only unused legal value is 0.
    sc = new;
    sc.skip = 2'd0;
    for (int i = 0; i < 3; i++) void'(sc.randomize());
    sc.skip = 2'd3;
    void'(sc.randomize());
    $display("continued=%0b", sc.r == 2'd0);
    $finish;
  end
endmodule
