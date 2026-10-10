// IEEE 1800-2009 20.15.1 L35315-35316: "The seed argument shall be an
// integral variable. The seed value should be assigned to this variable
// prior to calling $random." Annex N Table N.1 L76887: "$random
// rtl_dist_uniform (seed, LONG_MIN, LONG_MAX)"; N.2 L77212-77213:
// "if ((*seed) == 0) *seed = 259341593;".
// Decision (llg choice; the text does not give the initial value of the
// seed $random uses without an argument): it starts at 0, so the first calls
// return the Annex N values for seed 0. The values are fixed by Annex N.
module tb;
  integer r0, r1, r2;
  initial begin
    r0 = $random;
    r1 = $random;
    r2 = $random;
    $display("implicit %0d %0d %0d", r0, r1, r2);
    $finish;
  end
endmodule
