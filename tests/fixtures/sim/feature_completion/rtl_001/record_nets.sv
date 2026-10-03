// SV2009 sections 6.7, 7.2.2: recursive four-state record nets.
module tb;
  typedef struct { logic [7:0] lane [2:1]; logic signed [3:0] tag = 4'h7; } record_t;
  wire record_t value;
  wire record_t undriven;
  logic [7:0] first;
  logic enabled;
  assign value.lane[2] = first;
  assign value.lane[2] = enabled ? 8'hf0 : 8'hzz;
  assign value.lane[1] = 8'h55;
  assign value.tag = 4'hd;
  initial begin
    first = 8'h0f; enabled = 0;
    #1;
    $display("one=%h:%h:%0d undriven=%h:%b", value.lane[2], value.lane[1], value.tag, undriven.lane[2], undriven.tag);
    enabled = 1;
    #1; $display("conflict=%h other=%h", value.lane[2], value.lane[1]);
    first = 8'hzz;
    #1; $display("released=%h", value.lane[2]);
    $finish(0);
  end
endmodule
