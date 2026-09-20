// llg-test-fixture: tests/fixtures/sim/loops/foreach_mixed_too_many.sv
// IEEE 1800-2009 12.7.3: two dimensions cannot have three iterator slots.
module tb;
    logic [3:0] a [0:1];
    initial foreach (a[i,j,k]) ;
endmodule
