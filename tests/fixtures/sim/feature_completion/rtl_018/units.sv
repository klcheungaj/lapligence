// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/units.sv
// RTL-018 A02 (V 2001 §§13.2, 19.3-19.5; SV 2009 §§3.12.1, 22.4-22.6, 33.3):
// library sources follow the selected compilation-unit mode, map-local macros
// stay in their map, command-line defines seed every unit, and a library's
// -incdir header is found after the including directory and global roots.
module tb;
  wire [7:0] mark, shared, map_only, cli, late;
  rtl018_unit_a a(mark);
  rtl018_unit_b b(shared, map_only, cli, late);
  initial begin
    #1;
`ifdef RTL018_SHARED
    $display("tb sees shared");
`endif
    $display("mark=%0d shared=%0d map_only=%0d cli=%0d late=%0d", mark, shared,
             map_only, cli, late);
    $finish;
  end
endmodule
