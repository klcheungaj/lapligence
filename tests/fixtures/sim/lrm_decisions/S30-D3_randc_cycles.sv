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
// constraints are infeasible. A new cycle starts when the constraints on the
// variable change: constraint blocks switched on or off, a different inline
// constraint, or a different value of a state variable (or inline operand)
// that the constraints connected to the variable read, compared with the
// previous call; or when no unused value is legal. A state variable written
// and restored between two calls, one read only by constraints on unrelated
// variables, or the same constraint written at another call site changes
// nothing. A failed or inactive call does not
// advance the cycle.
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
    // A state change undone before the next call keeps the cycle: three
    // draws with skip == 0 still use 1, 2 and 3.
    sc = new;
    sc.skip = 2'd0;
    seen = 4'b0;
    void'(sc.randomize());
    seen[sc.r] = 1'b1;
    sc.skip = 2'd3;
    sc.skip = 2'd0;
    for (int i = 0; i < 2; i++) begin
      void'(sc.randomize());
      seen[sc.r] = 1'b1;
    end
    $display("restored state continues=%0b", seen == 4'b1110);
    // A changed state value starts a new cycle: after one draw with
    // skip == 0, the three draws with skip == 3 use 0, 1 and 2.
    sc = new;
    sc.skip = 2'd0;
    void'(sc.randomize());
    sc.skip = 2'd3;
    seen = 4'b0;
    for (int i = 0; i < 3; i++) begin
      void'(sc.randomize());
      seen[sc.r] = 1'b1;
    end
    $display("changed state restarts=%0b", seen == 4'b0111);
    // The same inline constraint at another call site continues the cycle.
    sc = new;
    sc.skip = 2'd3;
    seen = 4'b0;
    void'(sc.randomize() with { r != 2'd2; });
    seen[sc.r] = 1'b1;
    for (int i = 0; i < 1; i++) begin
      void'(sc.randomize() with { r != 2'd2; });
      seen[sc.r] = 1'b1;
    end
    $display("same inline elsewhere continues=%0b", seen == 4'b0011);
    $finish;
  end
endmodule
