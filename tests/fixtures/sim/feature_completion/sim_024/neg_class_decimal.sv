// SIM-024: a class handle is not an integral value; `%d` cannot format it
// (SV 21.2.1.2, 8.4).
module tb;
  class C;
  endclass
  C c;
  initial $display("%d", c);
endmodule
