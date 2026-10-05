`define LOOP for (genvar g = 0; g < 2; g = g + 1)
module tb;
  wire [1:0] w;
  generate
    `LOOP begin : bits
      assign w[g] = 1'b0;
    end
  endgenerate
endmodule
