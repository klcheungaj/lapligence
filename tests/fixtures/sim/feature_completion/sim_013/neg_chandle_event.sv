// SIM-013 A03 negative: chandles shall not be used in event expressions
// (SV 6.14), unlike class handles.
module tb;
  chandle x;
  initial begin
    @(x);
    $finish(0);
  end
endmodule
