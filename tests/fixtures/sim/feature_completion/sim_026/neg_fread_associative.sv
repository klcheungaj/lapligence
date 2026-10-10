// SIM-026 A03 negative: $fread into an associative array has no index order to load.
module tb;
  integer c, fd; logic [7:0] m[int];
  initial begin
    c = $fread(m, fd);
    $finish;
  end
endmodule
