// SIM-026 A03 negative: a string character destination is rejected explicitly (not supported).
module tb;
  integer c; string s;
  initial begin
    s = "ab";
    c = $sscanf("x", "%c", s[0]);
    $finish;
  end
endmodule
