// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/module_declarations.sv
// IEEE 1800-2009 §23.4: nested module definitions can resolve a parameter
// from their enclosing module instance.
module syn018_left #(parameter int BASE = 10) (
  input logic [3:0] a,
  output logic [3:0] y
);
  module leaf #(parameter int EXTRA = 1) (
    input logic [3:0] leaf_a,
    output logic [3:0] leaf_y
  );
    assign leaf_y = leaf_a + BASE + EXTRA;
  endmodule

  generate
    if (BASE == 10) begin : generated
      leaf #(.EXTRA(2)) u(a, y);
    end
  endgenerate
endmodule

// This second enclosing module intentionally declares another local `leaf`.
// The two definitions must remain independently scoped after elaboration.
module syn018_right #(parameter int BASE = 20) (
  input logic [3:0] a,
  output logic [3:0] y
);
  module leaf #(parameter int EXTRA = 1) (
    input logic [3:0] leaf_a,
    output logic [3:0] leaf_y
  );
    assign leaf_y = leaf_a + BASE + EXTRA;
  endmodule

  generate
    if (BASE == 20) begin : generated
      leaf #(.EXTRA(3)) u(a, y);
    end
  endgenerate
endmodule

module tb;
  logic [3:0] extern_a;
  logic [3:0] extern_y;
  logic [3:0] left_a;
  logic [3:0] left_y;
  logic [3:0] right_a;
  logic [3:0] right_y;

  syn018_extern_child #(.W(4)) ext(.a(extern_a), .y(extern_y));
  syn018_left #(.BASE(10)) left(left_a, left_y);
  syn018_right #(.BASE(20)) right(right_a, right_y);

  initial begin
    extern_a = 4'd3;
    left_a = 4'd1;
    right_a = 4'd1;
    #1 $display("extern=%0d nested=%0d/%0d", extern_y, left_y, right_y);
    $finish;
  end
endmodule
