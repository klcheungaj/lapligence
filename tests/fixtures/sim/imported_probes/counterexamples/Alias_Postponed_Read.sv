// Static-review counterexample; NOT EXECUTED.
module tb;
  wire [1:0] original;
  wire mirror;
  assign original = 2'b11;
  alias original[0] = mirror;
  initial begin
    #1;
    $strobe("alias=%b", mirror);
    #1; $finish(0);
  end
endmodule
