// `soft` constraints are IEEE 1800-2012; under sv2009 `soft` is not a
// keyword and the constraint does not parse (A03).
class c_c;
  rand bit [3:0] x;
  constraint c { soft x == 1; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
