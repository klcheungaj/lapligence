// SIM-030 A02: inheritance and inline name shadowing select the right
// fields (IEEE 1800-2009 8.14 name hiding, 18.5.2, 18.7).
class base_c;
  rand bit [3:0] x;
  randc bit [1:0] r;
  constraint cb { x == 4'd3; }
endclass

class derived_c extends base_c;
  rand bit [3:0] x;       // hides base_c::x
  constraint cd { x == 4'd9; r != 2'd0; }
endclass

module tb;
  derived_c d;
  base_c b;
  bit [3:0] x;            // a caller variable with the member's name
  bit [3:0] limit = 4'd12;
  bit [3:0] seen;
  int bad;

  initial begin
    d = new;
    b = d;
    // Each class's constraint names its own x; through either handle the
    // fields keep their meaning.
    void'(d.randomize());
    $display("hidden: derived=%0d base=%0d", d.x, b.x);
    // In an inline constraint a name resolves in the object first (18.7):
    // `x` is d.x, not the caller's x; `limit` is the caller's variable.
    x = 4'd1;
    bad = 0;
    for (int n = 0; n < 50; n++) begin
      void'(d.randomize() with { x < limit; });
      if (d.x != 9) bad = bad + 1;
    end
    $display("inline: bad=%0d caller x=%0d", bad, x);
    // The inherited randc variable is constrained by the derived class.
    for (int n = 0; n < 3; n++) begin
      void'(d.randomize());
      if (d.r == 0 || seen[d.r]) bad = bad + 1;
      seen[d.r] = 1'b1;
    end
    $display("inherited randc: bad=%0d seen=%b", bad, seen);
    $finish;
  end
endmodule
