// IEEE 1800-2009 11.4.14.4: a nonblocking unpack into a resizable container
// stays outside the fixed-destination slice (SIM-020) and keeps its diagnostic.
module tb;
  logic [7:0] dyn [];
  logic [7:0] h8;
  initial begin
    {>>{h8, dyn}} <= 24'h112233;
    #1 $finish;
  end
endmodule
