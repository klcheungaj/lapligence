// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/neg_bad_clock_witness.sv
// FND-002 witness neg_assert_bad_clock (L-F12-10-03, SV2009 16.13, 16.15):
// `intersect` of differently clocked sequences is illegal.
module tb;
bit c1=0,c2=0; sequence a; @(posedge c1) 1; endsequence sequence b; @(posedge c2) 1; endsequence assert property(a intersect b);
initial begin

$finish;
end
endmodule
