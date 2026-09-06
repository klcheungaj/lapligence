// llg-lsp-fixture: genvar/explicit.v
module GenvarExplicit;
  /* 😀 */ genvar /* declaration gap */
    count, spare;
  /* 😀 */ genvar extra; genvar last;
  generate
    for (count = 0; count < 2; count = count + 1) begin : lane
      wire [1:0] lane_value;
      assign lane_value = count;
    end
  endgenerate
  function integer passthrough;
    input integer count;
    begin
      passthrough = count;
    end
  endfunction
endmodule

module OrdinaryCounter;
  integer count;
  initial count = 1;
endmodule
