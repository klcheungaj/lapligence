// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/nested_out_of_scope.sv
// IEEE 1800-2009 §23.4: the local leaf is not visible outside its parent.
module syn018_owner;
  module leaf;
  endmodule
endmodule

module tb;
  leaf inaccessible();
endmodule
