// llg-lsp-fixture: lint-rules/src/careless_control.sv
// Exercises the shared control-flow lint batch and nearby quiet controls.
module careless_control (
  input  logic       a,
  input  logic       b,
  input  logic [1:0] sel,
  output logic       empty_result,
  output logic       condition_result,
  output logic       casex_result,
  output logic       quiet_result
);
  logic condition_lhs;

  // empty-implicit-sensitivity: constants contribute no signal sensitivity.
  always @(*)
    empty_result = 1'b0;

  // assignment-in-condition: this is an assignment expression, not `==`.
  always_comb begin
    if ((condition_lhs = b))
      condition_result = 1'b1;
    else
      condition_result = 1'b0;
  end

  // casex-statement: X and Z selector bits are both treated as wildcards.
  always_comb begin
    casex (sel)
      2'b1x: casex_result = 1'b1;
      default: casex_result = 1'b0;
    endcase
  end

  // Quiet controls: a real read gives @* sensitivity, an explicit comparison
  // is not assignment-in-condition, and exact case/casez are not casex.
  always @* begin
    quiet_result = a;
    if (a == b)
      quiet_result = 1'b1;
    case (sel)
      default: quiet_result = a;
    endcase
    casez (sel)
      default: quiet_result = b;
    endcase
  end
endmodule
