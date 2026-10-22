// Decision S2-D8: nets joined by alias statements are one physical net with
// one net delay. When members declare different delays, the delay of the
// first member (in alias-list order) that declares one applies to the whole
// net; the order of the declarations and the position of the driver do not
// matter. Across several alias statements the members count in the order
// they first appear (an llg choice: other simulators may pick by the driver's
// position there). A runtime-valued net delay must still be the only declared
// delay of its net (S2-D9).
//
// IEEE 1800-2009 10.11 (SystemVerilog-1800-2009.txt L13981, L13994-13995):
// "An alias statement declares multiple names for the same physical net, or
// bits within a net." "The members of an alias list are signals whose bits
// share the same physical nets." The standard gives a shared physical net no
// rule for choosing between different declared delays; following the first
// member is the industry-standard behaviour.
`timescale 1ns/1ns
module tb;
  reg a;
  // First member declares 4: the net delay is 4 (driver on the second).
  wire #4 p1;
  wire #6 p2;
  alias p1 = p2;
  assign p2 = a;
  // Same declarations, list reversed: the net delay is 6.
  wire #4 q1;
  wire #6 q2;
  alias q2 = q1;
  assign q2 = a;
  // The first member declares no delay: the next member's 3 applies.
  wire r1;
  wire #3 r2;
  wire #5 r3;
  alias r1 = r2 = r3;
  assign r3 = a;
  // Rise and fall delays come from the first member as a whole: rise 2,
  // fall 8.
  wire #(2, 8) s1;
  wire #(6, 3) s2;
  alias s1 = s2;
  assign s2 = a;
  initial begin
    $monitor("%0t p=%b q=%b r=%b s=%b", $time, p1, q1, r1, s1);
    a = 1'b0;
    #20 a = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
endmodule
