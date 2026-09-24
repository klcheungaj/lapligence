// IEEE 1800-2009 11.9: dot writes must check the active tag too.
module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } tagged_t;
  tagged_t value;
  initial begin
    value = tagged A(8'h11);
    value.B = 8'h22;
    $display("AFTER_INACTIVE_WRITE active_A=%h", value.A);
    $finish;
  end
endmodule
