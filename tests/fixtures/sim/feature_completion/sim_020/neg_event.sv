// SIM-020 A03: an event is not a bit-stream type.
module tb;
  event e;
  logic [63:0] v;
  initial begin
    v = {>>{e}};
    $finish;
  end
endmodule
