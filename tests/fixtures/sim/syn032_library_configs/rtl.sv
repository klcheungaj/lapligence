// llg-test-fixture: tests/fixtures/sim/syn032_library_configs/rtl.sv
module logic_cell #(parameter integer VALUE = 0)();
  localparam integer LIB_MARK = 11;
  initial $display("cell=%0d value=%0d", LIB_MARK, VALUE);
endmodule

module default_cell();
  initial $display("default=%0d", 11);
endmodule
