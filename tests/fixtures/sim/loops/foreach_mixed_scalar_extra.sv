// llg-test-fixture: tests/fixtures/sim/loops/foreach_mixed_scalar_extra.sv
// A scalar element does not create an additional foreach dimension.
module tb;
    logic a [0:1];
    initial foreach (a[i,j]) ;
endmodule
