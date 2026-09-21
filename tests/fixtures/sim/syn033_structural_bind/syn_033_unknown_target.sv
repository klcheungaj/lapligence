// llg-test-fixture: tests/fixtures/sim/syn033_structural_bind/syn_033_unknown_target.sv
// IEEE 1800-2009 §23.11: the target name is intentionally unknown; this is
// the single structural-bind diagnostic control.
module syn033_unknown_probe(input logic a);
endmodule

module tb;
  logic a;
  initial begin
    a = 1'b0;
    #1;
    $finish;
  end
endmodule

bind syn033_missing_target syn033_unknown_probe bad(.a(a));
