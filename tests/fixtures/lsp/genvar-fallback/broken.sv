// llg-lsp-fixture: genvar-fallback/broken.sv
module GenvarFallback;
  genvar index;
  for (index = 0; index < 2; index = index + 1) begin : old_loop
    if (1) begin : wire_scope
      wire index;
      assign index = 1'b1;
      wire scoped_value = index;
    end
    wire [1:0] data = index;
  end
  for (genvar row = 0; row < 2; row++) begin : new_loop
    wire [1:0] data = row;
  end
endmodule

module BrokenSibling;
  wire broken = ;
endmodule
