// A value-form randomize() in a short-circuit operand would run even when
// the operand is skipped; it is rejected rather than reordered.
class c_c; rand bit [3:0] a; endclass
module tb;
  bit en;
  initial begin
    static c_c c = new;
    if (en && c.randomize()) $display("done");
    $finish;
  end
endmodule
