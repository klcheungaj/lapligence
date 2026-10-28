// Definitions for assertion_names.sv, declared in another project file
// (ST-75 in docs/semantic_tokens.md).
interface an_bus_if;
  logic req;
endinterface

module an_dut(input logic clk, input logic [1:0] sel);
  logic bridge_mode;
  logic [1:0] intreg_flash_select;
  assign bridge_mode = clk;
  assign intreg_flash_select = sel;
endmodule
