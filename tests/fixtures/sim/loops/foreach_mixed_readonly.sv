// llg-test-fixture: tests/fixtures/sim/loops/foreach_mixed_readonly.sv
// IEEE 1800-2009 12.7.3: implicit foreach iterator variables are read-only.
module tb;
    logic [3:0] a [0:1];
    initial foreach (a[i,j]) j = 0;
endmodule
