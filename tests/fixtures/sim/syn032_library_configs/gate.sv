// llg-test-fixture: tests/fixtures/sim/syn032_library_configs/gate.sv
module logic_cell #(parameter integer VALUE = 0)();
  localparam integer LIB_MARK = 22;
  initial $display("cell=%0d value=%0d", LIB_MARK, VALUE);
endmodule

config gate_cfg;
  design gate.logic_cell;
endconfig
