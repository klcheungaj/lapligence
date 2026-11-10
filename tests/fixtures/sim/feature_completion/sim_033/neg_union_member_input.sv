`timescale 1ns/1ns
// An unpacked struct clockvar with an unpacked union member rejects
// (decision K1-D1).
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  typedef union { int i; bit [31:0] u; } un_t;
  typedef struct { byte b; un_t un; } st_t;
  st_t st;
  clocking cb @(posedge clk);
    input st;
  endclocking
  initial begin
    st.b = 1;
    @(cb) $display("%0d", cb.st.b);
    $finish;
  end
endmodule
