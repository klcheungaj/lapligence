// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2012_bins_set.sv
// IEEE 1800-2009 A.2.11: bins take { ranges }, a transition list or default;
// set-expression bins are an IEEE 1800-2012 form.
module tb;
  bit [3:0] v;
  int values[2] = '{1, 2};
  covergroup cg;
    coverpoint v { bins chosen = values; }
  endgroup
  initial $finish;
endmodule
