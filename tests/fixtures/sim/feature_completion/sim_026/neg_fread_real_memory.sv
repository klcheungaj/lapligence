// SIM-026 A03 negative: $fread loads integral memories only (IEEE 1800-2009 21.3.4.4).
module tb;
  integer c, fd; real m [2];
  initial begin
    c = $fread(m, fd);
    $finish;
  end
endmodule
