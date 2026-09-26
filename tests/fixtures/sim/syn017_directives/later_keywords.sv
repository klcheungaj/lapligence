// IEEE 1800-2009 22.14; lexical keyword switches postdate 1364-2001.
`begin_keywords "1364-2001"
module tb;
  reg value;
  initial begin
    value = 1'b1;
    $display("keywords=%b", value);
    $finish;
  end
endmodule
`end_keywords
