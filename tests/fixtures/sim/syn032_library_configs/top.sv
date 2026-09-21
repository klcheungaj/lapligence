// llg-test-fixture: tests/fixtures/sim/syn032_library_configs/top.sv
module top;
  logic_cell #(.VALUE(3)) from_cell();
  logic_cell #(.VALUE(4)) from_instance();
  default_cell from_default();
endmodule
