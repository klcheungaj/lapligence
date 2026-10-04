// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/witness_missing_incdir.v
// Adopted FND-002 witness library_missing_include (L-F03-08-03, L-F03-08-04).
// V2001 13.2; SV2009 33.3; owner P-05.
// Expected: admission diagnostic for the nonexistent include directory.
module tb; initial $finish; endmodule
