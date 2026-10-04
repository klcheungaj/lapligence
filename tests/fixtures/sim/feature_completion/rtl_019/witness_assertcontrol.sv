// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/witness_assertcontrol.sv
// Adopted FND-002 witness neg_post2009_assertcontrol (L-F12-10-09).
// SV2009 20.11-20.12: $assertcontrol is an IEEE 1800-2012 task.
// Expected: strict edition rejection of `$assertcontrol`.
module tb;

initial begin
$assertcontrol(3);
$finish;
end
endmodule
