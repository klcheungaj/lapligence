// SIM-026 A03 negative: $fgets into a real is rejected by the frontend.
module tb;
  integer c, fd; real r;
  initial begin
    c = $fgets(r, fd);
    $finish;
  end
endmodule
