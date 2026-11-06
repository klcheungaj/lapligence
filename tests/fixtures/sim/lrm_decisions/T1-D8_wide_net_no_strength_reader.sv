// Decision T1-D8: only a reader of a net's strength needs its view, so a net
// too wide for a view is served while nothing waits on it.
module tb;
  tri1 [131071:0] w;
  initial begin
    #1 $display("w0=%b", w[0]);
    $finish(0);
  end
endmodule
