// IEEE 1800-2009 20.15.2 L35355-35356: "the seed argument is an inout
// argument"; Annex N.2 reads it as the 2-state C `long *seed`. The text does
// not say how a seed with X or Z bits is read.
// Decision (llg choice): a seed whose low 32 bits (the bits read as the
// seed, S28-D2) hold any X or Z bit reads as 0 as a whole. Annex N: seed 0
// -> 303379748, seed -1844104698; $dist_uniform(seed 0, 0, 10) -> 6, seed
// -1844104698.
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
