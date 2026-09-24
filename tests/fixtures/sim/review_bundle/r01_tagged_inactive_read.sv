// IEEE 1800-2009 11.9: a tag-inconsistent read requires a runtime error.
module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } tagged_t;
  tagged_t value;
  logic [7:0] got;
  initial begin
    value = tagged A(8'h11);
    got = value.B;
    $display("AFTER_INACTIVE_READ got=%h", got);
    $finish;
  end
endmodule
