// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/line_units.sv
// IEEE 1800-2009 22.12: `line maps only the rest of its own file. In merged
// and separate compilation units this file still reports physical positions.
module tb;
  peer p();
  initial begin
    $display("%s:%0d", `__FILE__, `__LINE__);
    p.show;
    $finish;
  end
endmodule
