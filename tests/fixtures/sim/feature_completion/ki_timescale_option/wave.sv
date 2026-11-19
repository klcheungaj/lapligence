// Waveform header and change times under the default time scale.
module tb;
  logic a = 1'b0;
  initial begin
    #1 a = 1'b1;
    #1 $finish;
  end
endmodule
