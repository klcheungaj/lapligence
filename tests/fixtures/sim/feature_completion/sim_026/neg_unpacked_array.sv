// SIM-026 A03 negative: an integral conversion into an unpacked int array is illegal (IEEE 1800-2009 21.3.4.3).
module tb;
  integer c; int ua [2];
  initial begin
    c = $sscanf("1", "%d", ua);
    $finish;
  end
endmodule
