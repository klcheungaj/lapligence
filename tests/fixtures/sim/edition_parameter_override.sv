module tb #(parameter int P = 1)();
  generate
    if (P == 2) begin : yes
    end
  endgenerate
endmodule
