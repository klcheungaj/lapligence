// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/nested_2001.sv
// IEEE 1364-2001 Annex A.1.5 does not admit module declarations as module items.
module syn018_owner;
  module leaf;
  endmodule
  leaf u();
endmodule

module tb;
  syn018_owner owner();
endmodule
