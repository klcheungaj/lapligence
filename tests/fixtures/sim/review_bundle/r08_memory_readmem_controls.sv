// llg-test-fixture: tests/fixtures/sim/review_bundle/r08_memory_readmem_controls.sv
module tb;
  logic [1:0] packed_memory[0:1];
  typedef enum logic signed [1:0] { NEGATIVE_ONE = -2'sd1, ZERO = 2'sd0 } signed_enum_t;
  signed_enum_t signed_memory[0:1];
  typedef enum bit [1:0] { BIT_ZERO = 2'd0, BIT_ONE = 2'd1 } bit_enum_t;
  bit_enum_t bit_memory[0:1];
  initial begin
    $readmemh("enum_overflow.hex", packed_memory);
    $display("PACKED_CONTROL %0d %0d", packed_memory[0], packed_memory[1]);
    $readmemh("signed_enum.hex", signed_memory);
    $display("SIGNED_ENUM_CONTROL %0d", signed_memory[0]);
    $readmemh("signed_enum_nonextension.hex", signed_memory);
    $display("SIGNED_ENUM_AFTER_NONEXTENSION %0d %0d", signed_memory[0], signed_memory[1]);
    $readmemh("enum_unknown.hex", bit_memory);
    $display("TWO_STATE_ENUM_CONTROL %0d %0d", bit_memory[0], bit_memory[1]);
    $finish;
  end
endmodule
