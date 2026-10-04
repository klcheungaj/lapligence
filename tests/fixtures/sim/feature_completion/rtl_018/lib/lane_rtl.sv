// RTL-018 library `rtl` lane: reads its image cell by ID.
module rtl018_lane #(parameter int GAIN = 1, parameter int ID = 0) (
  input logic [7:0] x,
  input logic [8:0] image [131072],
  output logic [7:0] y
);
  assign y = 8'(x * GAIN + image[ID]);
  initial #(10 + ID) $display("lane %l GAIN=%0d", GAIN);
endmodule
