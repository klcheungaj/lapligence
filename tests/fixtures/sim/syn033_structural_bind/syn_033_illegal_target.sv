// llg-test-fixture: tests/fixtures/sim/syn033_structural_bind/syn_033_illegal_target.sv
// IEEE 1800-2009 §23.11: a primitive is not a legal bind target; this is
// the single illegal-target diagnostic control.
module syn033_illegal_probe(input logic a);
endmodule

module tb;
  logic a;
  logic y;
  and gate0(y, a, a);
  initial begin
    a = 1'b0;
    #1;
    $finish;
  end
endmodule

bind tb.gate0 syn033_illegal_probe bad(.a(a));
