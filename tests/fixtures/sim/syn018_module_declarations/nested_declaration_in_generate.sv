// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/nested_declaration_in_generate.sv
// IEEE 1800-2009 Annex A.1.4 and A.4.2: a module declaration is a
// non-port module item, but it is not a generate item.
module tb;
  generate
    if (1) begin : generated
      module leaf;
      endmodule
    end
  endgenerate
endmodule
