// SIM-026 negative: the coverage database names take the string type.
module tb;
  reg [63:0] value = "covdb";
  initial begin
    $set_coverage_db_name(value);
    $finish;
  end
endmodule
