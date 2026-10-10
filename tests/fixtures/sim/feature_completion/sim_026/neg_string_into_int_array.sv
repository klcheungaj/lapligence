// SIM-026 A03 negative: %s into an unpacked array of int (not byte) is illegal (IEEE 1800-2009 21.3.4.3).
module tb;
  integer c; int ua[2];
  initial begin
    c = $sscanf("1", "%s", ua);
    $finish;
  end
endmodule
