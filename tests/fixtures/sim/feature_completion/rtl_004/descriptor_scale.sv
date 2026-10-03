// SV2009 10.9.1: sparse defaults and integer keys at the 16M storage budget.
`ifndef PATTERN_CELLS
`define PATTERN_CELLS 65537
`endif
module tb;
  localparam N = `PATTERN_CELLS;
  logic [16:0] values[N-1:0];
  initial begin
    values = '{default:17'h13579, N-1:17'h1aaaa, 0:17'h1bbbb};
    if(values[N-1]!==17'h1aaaa || values[0]!==17'h1bbbb || values[N/2]!==17'h13579) $fatal(1,"large pattern");
    $display("descriptor_scale=pass");
    $finish(0);
  end
endmodule
