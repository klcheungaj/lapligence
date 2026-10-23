// Decision A9-D13: a negative limit of a check whose limit is a
// non-negative constant expression counts as zero (the frontend warns), so
// the check never reports.
//
// IEEE 1800-2009 31.3.1 Table 31-1 (SystemVerilog-1800-2009.txt L50962):
// "limit Non-negative constant expression"; L50987-50988: "When the limit is
// zero, the $setup check shall never issue a violation."
`timescale 1ns/1ns
module su(input d, input c); reg n; specify $setup(d, posedge c, -5, n); endspecify endmodule
module tb;
  reg c, d;
  su s(d, c);
  initial $monitor("%0t n=%b", $time, s.n);
  initial begin
    c = 0; d = 0;
    #99 d = 1;
    #1 c = 1;
    #10 $finish;
  end
endmodule
