// SV2009 10.9.1: immediate packed element types and explicit-index precedence.
module tb;
  bit [1:0] adopted;
  logic [0:3] ascending;
  bit [3:0] binary;
  logic [1:0][2:0] rows;
  typedef logic [2:0] row_t;
  initial begin
    adopted = '{bit:1};
    ascending = '{logic:1'bx, 1:1'bz, default:1'b0};
    binary = '{bit:1'bx, 2:1'b1};
    rows = '{row_t:3'b101, 0:3'b011};
    $display("%b %b %b %b", adopted, ascending, binary, rows);
    $finish(0);
  end
endmodule
