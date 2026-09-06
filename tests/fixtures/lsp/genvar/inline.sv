// llg-lsp-fixture: genvar/inline.sv
module GenvarInline;
  localparam integer idx = 7;
  for (genvar idx = 0; idx < 2; idx++) begin : outer_loop
    wire [1:0] outer_value = idx;
    for (genvar inner = 0; inner < 2; inner++) begin : inner_loop
      wire [2:0] inner_value = inner + idx;
    end
  end
  for (genvar idx = 0; idx < 0; idx++) begin : pruned_loop
    wire [1:0] pruned_value = idx;
  end
  wire [3:0] after_loop = idx;
endmodule
