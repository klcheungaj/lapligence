// IEEE 1800-2009 section 22.5.1: a macro that expands to a SystemVerilog
// process keyword is accepted in the 2009 profile and rejected in the 2001
// profile.
`define SYN017_ALWAYS always_comb
module tb;
  reg x;
  `SYN017_ALWAYS begin
    x = 1'b1;
  end
  initial begin
    #0;
    $display("x=%b", x);
    $finish;
  end
endmodule
