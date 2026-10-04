// Header for owned_error_include.sv: the fork on line 4 is the error.
always_ff @(posedge clk) begin
  fork
    q <= 1'b1;
  join
end
