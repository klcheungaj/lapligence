// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/witness_empty_incdir.v
// Adopted FND-002 witness library_empty_include (L-F03-08-05).
// V2001 13.2; SV2009 33.3. An include directory without headers is legal.
// Expected: library
module tb; child c(); initial #1 $finish; endmodule
