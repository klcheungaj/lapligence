// IEEE 1800-2009 20.15.2 L35355-35356: "the seed argument is an inout
// argument"; Annex N.2 reads it as the 2-state C `long *seed`. 6.11.2
// L5489-5490: "When a 4-state value is automatically converted to a 2-state
// value, any unknown or high-impedance bits shall be converted to zeros."
// Decision (llg choice; the text does not say how an X/Z seed is read): X/Z
// seed bits read as 0, as in a 4-state to 2-state conversion. Annex N: seed
// 0 -> 303379748, seed -1844104698; seed 1 -> -2147414528, seed 69070;
// $dist_uniform(seed 0, 0, 10) -> 6, seed -1844104698.
module tb;
  integer seed, r;
  initial begin
    seed = 'x;
    r = $random(seed);
    $display("unknown %0d %0d", r, seed);
    seed = 32'b0000_0000_0000_0000_0000_0000_0000_00z1;
    r = $random(seed);
    $display("partly unknown %0d %0d", r, seed);
    seed = 'x;
    r = $dist_uniform(seed, 0, 10);
    $display("uniform %0d %0d", r, seed);
    $finish;
  end
endmodule
