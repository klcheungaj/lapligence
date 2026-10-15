// Uniqueness constraints are IEEE 1800-2012 and rejected under sv2009 (A03).
class c_c;
  rand bit [3:0] x, y;
  constraint c { unique {x, y}; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
