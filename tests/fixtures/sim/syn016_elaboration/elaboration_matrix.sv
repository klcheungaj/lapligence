// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/elaboration_matrix.sv
// IEEE 1800-2009 §§3.12.1, 6.18–6.20.3, 6.23–6.24.1, 11.2.1, 11.13, 25.8 and
// 26.2–26.6: finite constant elaboration across $unit, packages, types,
// generate scopes and parameterized interfaces.
localparam int UNIT_BIAS = 7;
function automatic int unit_bump(input int value);
  unit_bump = value + UNIT_BIAS;
endfunction

package syn016_base_pkg;
  parameter int BASE = 3;
  typedef logic signed [BASE:0] word_t;
  typedef enum logic [1:0] {ZERO = 0, ONE = 1, TWO = 2} state_t;

  function automatic int bump(input int value);
    bump = value + BASE;
  endfunction

  let twice(x) = x + x;
endpackage

package syn016_facade_pkg;
  import syn016_base_pkg::*;
  export syn016_base_pkg::*;
endpackage

interface syn016_bus #(parameter int W = 4);
  logic [W-1:0] data;
endinterface

module syn016_child #(
    parameter type T = logic [3:0],
    parameter int W = $bits(T),
    parameter int OFFSET = 1
) (
    syn016_bus bus,
    output logic [W-1:0] out
);
  import syn016_facade_pkg::*;
  typedef T alias_t;
  typedef type(out) output_t;
  localparam int LEFT = $left(alias_t);
  localparam int RIGHT = $right(alias_t);
  localparam int DIM = width_fn(2);
  localparam real SCALE = 1.5 * 4.0;
  localparam int REAL_WIDTH = int'(SCALE);
  localparam int PACKAGE_BITS = $bits(word_t);
  localparam int TYPE_BITS = $bits(output_t);
  localparam string LABEL = {"syn", "016"};
  localparam state_t STATE = TWO;
  logic [DIM-1:0] generated;

  function automatic int width_fn(input int value);
    width_fn = value * 2;
  endfunction

  generate
    if (W == 4) begin : narrow
      assign generated = {DIM{1'b1}};
    end else begin : wide
      assign generated = {DIM{1'b0}};
    end
  endgenerate

  assign out = bus.data + W + OFFSET + LEFT + RIGHT + BASE + int'(STATE)
             + REAL_WIDTH + PACKAGE_BITS + TYPE_BITS + unit_bump(1);
  initial $display("child=%0d label=%s dim=%0d", W, LABEL, DIM);
endmodule

module tb import syn016_facade_pkg::*; ();
  syn016_bus #(4) b0();
  syn016_bus #(8) b1();
  logic [3:0] out0;
  logic [7:0] out1;

  syn016_child #(.T(logic [3:0]), .OFFSET(2)) c0(b0, out0);
  syn016_child #(.T(logic signed [7:0]), .OFFSET(2)) c1(b1, out1);

  initial begin
    b0.data = 4'h1;
    b1.data = 8'ha5;
    #1 $display("out=%h/%h bits=%0d/%0d base=%0d bump=%0d twice=%0d",
               out0, out1, $bits(out0), $bits(out1), BASE, bump(2), twice(3));
    $finish;
  end
endmodule
