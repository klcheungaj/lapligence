// S30-D15 (18.3): constraints are expressions of integral type, so a whole
// unpacked array has no place in one, even compared with another.
class c_c;
  rand bit [3:0] a [4];
  bit [3:0] b [4];
  constraint c { a == b; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish(0);
  end
endmodule
