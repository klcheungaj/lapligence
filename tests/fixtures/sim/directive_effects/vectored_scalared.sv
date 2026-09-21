// IEEE 1364-2001 section 3.3.2 and IEEE 1800-2009 section 6.9:
// vectored/scalared declarations retain their simulation-neutral packed
// values.
module tb;
  wire vectored [3:0] v;
  wire scalared [3:0] s;
  assign v = 4'ha;
  assign s = 4'h5;
  initial begin
    #0;
    $display("v=%h s=%h", v, s);
    $finish;
  end
endmodule
