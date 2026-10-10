// SIM-026 A03 negative: a constant is not a variable (rejected by the frontend).
module tb;
  integer c; localparam int P = 1;
  initial begin
    c = $sscanf("1", "%d", P);
    $finish;
  end
endmodule
