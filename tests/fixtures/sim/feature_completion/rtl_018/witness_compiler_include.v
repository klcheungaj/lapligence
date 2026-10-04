// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/witness_compiler_include.v
// Adopted FND-002 witness library_compiler_include (L-F03-08-01).
// V2001 13.2.2 versus Annex A; SV2009 33.3.2 versus Annex A; owner N12 P-05.
// Expected: owner diagnostic, a compiler `include directive is not admitted in maps.
module tb; initial $finish; endmodule
