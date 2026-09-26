// IEEE 1800-2009 22.5.1; `" is not a 1364-2001 macro operator.
`define SYN017_TEXT(x) `"x`"
module tb;
  initial begin
    $display("quote=%s", `SYN017_TEXT(value));
    $finish;
  end
endmodule
