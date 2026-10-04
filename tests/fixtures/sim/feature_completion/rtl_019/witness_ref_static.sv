// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/witness_ref_static.sv
// Adopted FND-002 witness neg_ref_static (L-F09-03-05).
// SV2009 13.5.2: a ref formal of a static subroutine is illegal.
// Expected: blocking frontend rejection (no longer only a warning).
module tb;
int x; task t(ref int a); a=1; endtask
initial begin
t(x);
$finish;
end
endmodule
