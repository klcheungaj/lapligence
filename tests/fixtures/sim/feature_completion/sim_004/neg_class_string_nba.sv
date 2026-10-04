// SIM-004 negative: class properties are members of dynamic objects and are
// not nonblocking targets (IEEE 1800-2009 6.21).
module tb;
  class C;
    string tag;
  endclass
  C object;
  initial begin
    object = new;
    object.tag <= "x";
    #1 $display("%s", object.tag);
    $finish(0);
  end
endmodule
