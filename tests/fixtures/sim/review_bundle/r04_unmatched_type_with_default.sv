// Unmatched type keys do not invalidate elements covered by default.
module tb;
  typedef logic [7:0] lane_t;
  typedef logic signed [7:0] signed_lane_t;
  lane_t values[0:1];
  lane_t seed;
  initial begin
    seed = 8'h22;
    values = '{signed_lane_t:8'sh11, default:seed};
    if (values[0] !== 8'h22 || values[1] !== 8'h22) $fatal(1, "default");
    $display("PASS r04_unmatched_type_with_default");
    $finish;
  end
endmodule
