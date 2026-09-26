// IEEE 1800-2009 22.11; this annotation has no simulator data effect.
`pragma diagnostic push
module tb;
  reg value;
  initial begin
    value = 1'b1;
    $display("pragma=%b", value);
    $finish;
  end
endmodule
`pragma diagnostic pop
