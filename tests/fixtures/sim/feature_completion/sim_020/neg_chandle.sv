// SIM-020 A03: a chandle is not a bit-stream type.
module tb;
  chandle h;
  logic [63:0] v;
  initial begin
    v = {>>{h}};
    $finish;
  end
endmodule
