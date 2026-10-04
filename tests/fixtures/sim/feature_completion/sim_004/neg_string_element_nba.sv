// SIM-004 negative: a string is a dynamically sized variable, so its bytes
// are not nonblocking targets (IEEE 1800-2009 6.16, 6.21).
module tb;
  string s;
  initial begin
    s = "ab";
    s[0] <= "x";
    #1 $display("%s", s);
    $finish(0);
  end
endmodule
