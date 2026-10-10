// IEEE 1800-2009 20.15.1 L35315: "The seed argument shall be an integral
// variable." 20.15.2 L35355-35356: "the seed argument is an inout argument;
// that is, a value is passed to the function, and a different value is
// returned." Annex N.2 declares the seed as the 32-bit `long *seed`.
// Decision (llg choice; the text does not say how a seed variable of
// another width converts): the variable is read as a 32-bit integer
// (truncated, or extended by its own signedness) and the signed 32-bit result
// seed is assigned back like an integer (sign-extended or truncated).
// Annex N: seed 1 -> -2147414528, seed 69070; seed -1 -> 2147415551, seed
// -69068 (32'hfffef234).
module tb;
  integer r;
  time t;
  reg [47:0] r48;
  reg signed [15:0] s16;
  reg [15:0] u16;
  initial begin
    t = 64'h0000_0000_ffff_ffff;
    r = $random(t);
    $display("time %0d %h", r, t);
    r48 = 48'hffff_0000_0001;
    r = $random(r48);
    $display("reg48 %0d %h", r, r48);
    s16 = -1;
    r = $random(s16);
    $display("signed16 %0d %0d", r, s16);
    u16 = 1;
    r = $random(u16);
    $display("unsigned16 %0d %0d", r, u16);
    $finish;
  end
endmodule
