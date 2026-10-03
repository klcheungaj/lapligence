// IEEE 1800-2009 6.24.3: an unpacked union is not a bit-stream type.
module tb;
  typedef union { logic [7:0] a; logic [7:0] b; } u_t;
  u_t u;
  initial begin
    u = u_t'(8'h12);
    $finish;
  end
endmodule
