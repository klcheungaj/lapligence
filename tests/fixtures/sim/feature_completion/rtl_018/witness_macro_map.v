// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/witness_macro_map.v
// Adopted FND-002 witness library_macro_map (L-F03-08-06).
// V2001 13.2, 19.3-19.5; SV2009 33.3, 22.4-22.6; retained N12 grammar policy.
// Expected: library
module tb; child c(); initial #1 $finish; endmodule
