// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2012_bins_with.sv
// IEEE 1800-2009 A.2.11: bins_or_options has no `with` clause (an IEEE
// 1800-2012 form); nearest legal: bins b[] = {[0:15]}.
module tb;
  bit [3:0] v;
  covergroup cg;
    coverpoint v { bins odd[] = {[0:15]} with (item % 2 == 1); }
  endgroup
  initial $finish;
endmodule
