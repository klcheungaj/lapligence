// An untyped parameter takes the type of a real override, so `$bits` of it is
// the width of a real (IEEE 1800-2009 23.10, 6.12).
module tb #(
  parameter ANY = 0
);
  initial $display("ANY=%g bits=%0d", ANY, $bits(ANY));
endmodule
