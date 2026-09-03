// llg-lsp-fixture: lint-rules/src/careless_more.sv
// Exercises the expanded shared careless-mistake lint rules end to end.
module careless_more (
  input  logic       a,
  input  logic       b,
  input  logic [7:0] bus,
  input  logic [1:0] sel,
  output logic       sensitivity_result,
  output logic       case_result,
  output logic       floating_result,
  output logic       range_result,
  output logic       xz_result
);
  logic floating;

  // undriven-signal: `floating` is consumed but has no source.
  assign floating_result = floating;

  // incomplete-sensitivity-list: changes to `b` must also wake this block.
  always @(a)
    sensitivity_result = a & b;

  // out-of-range-select: bus indices are 7 through 0.
  assign range_result = bus[8];

  // xz-logical-equality: logical equality does not test for unknown values.
  assign xz_result = (a == 1'bx);

  // duplicate-case-item: the later 2'd1 arm is unreachable.
  always_comb begin
    case (sel)
      2'd0: case_result = 1'b0;
      2'd1: case_result = 1'b1;
      2'd1: case_result = a;
      default: case_result = b;
    endcase
  end
endmodule
