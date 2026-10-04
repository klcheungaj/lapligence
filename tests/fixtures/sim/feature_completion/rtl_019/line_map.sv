// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/line_map.sv
// IEEE 1800-2009 22.12-22.13: `__FILE__/`__LINE__ follow nested includes,
// restore after them and follow `line. Values in line_map.out are counted by hand.
`define HERE $display("%s:%0d", `__FILE__, `__LINE__)
module tb;
  initial begin
    `HERE;
`include "line_map_outer.svh"
    `HERE;
`line 300 "mapped_top.sv" 0
    `HERE;
`include "line_map_outer.svh"
    `HERE;
    $finish;
  end
endmodule
