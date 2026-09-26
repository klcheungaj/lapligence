// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/nested_specializations.sv
// IEEE 1800-2009 §23.4: each instance specializes its local nested definition
// in the enclosing parameter context, including the nested ports and default.
module syn018_parent #(parameter int W = 4, BIAS = 1) (
  input logic [W-1:0] a,
  output logic [W-1:0] y
);
  module leaf #(parameter int EXTRA = BIAS) (
    input logic [W-1:0] leaf_a,
    output logic [W-1:0] leaf_y
  );
    assign leaf_y = leaf_a + EXTRA;
  endmodule

  leaf u(.leaf_a(a), .leaf_y(y));
endmodule

module tb;
  logic [3:0] a4, y4;
  logic [4:0] a5, y5;
  syn018_parent #(.W(4), .BIAS(1)) p4(.a(a4), .y(y4));
  syn018_parent #(.W(5), .BIAS(3)) p5(.a(a5), .y(y5));

  initial begin
    a4 = 4'd3;
    a5 = 5'd3;
    #1 $display("specialized=%0d/%0d widths=%0d/%0d", y4, y5, $bits(y4), $bits(y5));
    $finish;
  end
endmodule
