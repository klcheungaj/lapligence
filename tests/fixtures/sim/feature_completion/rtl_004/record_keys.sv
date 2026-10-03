// SV2009 10.9.1: explicit member, last matching type, recursive default.
module tb;
  typedef logic [2:0] lane_t;
  typedef bit [2:0] binary_t;
  typedef struct { binary_t binary; lane_t four; logic [64:0] wide; } record_t;
  typedef lane_t row_t[-1:0];
  record_t value;
  lane_t matrix[2:1][-1:0];
  logic [64:0] vector;
  initial begin
    value = '{binary_t:3'bxz1, lane_t:3'b001, lane_t:3'bz10,
              wide:65'h10000000000000001, default:'0};
    matrix = '{row_t:'{3'b001,3'b010}, row_t:'{3'b101,3'b110}, 1:'{default:3'bz01}};
    vector = '{logic:1'b1, 0:1'bz, 64:1'bx};
    $display("%b %b %h", value.binary, value.four, value.wide);
    $display("%b %b %b %b", matrix[2][-1],matrix[2][0],matrix[1][-1],matrix[1][0]);
    $display("%b %b %b", vector[64],vector[63:1],vector[0]);
    $finish(0);
  end
endmodule
