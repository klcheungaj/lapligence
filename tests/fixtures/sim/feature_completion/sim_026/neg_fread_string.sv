// SIM-026 A03 negative: $fread into a string is rejected by the frontend.
module tb;
  integer c, fd; string s;
  initial begin
    c = $fread(s, fd);
    $finish;
  end
endmodule
