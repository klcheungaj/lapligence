// SV 16.6.1: $sampled of a string is rejected rather than read as a value.
module tb;
  string s = "a";
  initial begin
    #1 $display("%s", $sampled(s));
    $finish;
  end
endmodule
