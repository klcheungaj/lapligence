// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/bind_generate.sv
// RTL-018 A03 negative (SV 2009 §23.11): a generate block path names no
// module or interface instance.
module rtl018_obs; endmodule
module rtl018_target; endmodule
module tb;
  for (genvar i = 0; i < 1; i++) begin : rows
    rtl018_target t();
  end
  initial $finish;
endmodule
bind tb.rows[0] rtl018_obs o();
