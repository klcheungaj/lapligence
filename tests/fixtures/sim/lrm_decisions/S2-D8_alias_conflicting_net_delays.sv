// Decision S2-D8: nets joined by an alias statement that declare different
// net delays are rejected when the design is compiled. Equal delays (or a
// delay on one member only) are one net delay of the shared net, applied
// once.
//
// IEEE 1800-2009 10.11 (SystemVerilog-1800-2009.txt L13981, L13994-13995):
// "An alias statement declares multiple names for the same physical net, or
// bits within a net." "The members of an alias list are signals whose bits
// share the same physical nets." The standard gives a shared physical net no
// rule for choosing between different declared delays (the port-collapse
// rule of 23.3.3.7 does not apply to aliases), so llg rejects the conflict (an
// llg choice). The expected output is empty: the design does not compile.
`timescale 1ns/1ns
module tb;
  reg a;
  wire #4 w1;
  wire #6 w2;
  alias w1 = w2;
  assign w2 = a;
  initial begin
    $monitor("%0t w1=%b w2=%b", $time, w1, w2);
    a = 1'b0;
    #10 a = 1'b1;
    #10 $finish;
  end
endmodule
