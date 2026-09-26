// IEEE 1800-2009 22.5.1; `` is not a 1364-2001 macro operator.
`define SYN017_NAME(x) x``_field
module tb;
  reg value_field;
  initial begin
    `SYN017_NAME(value) = 1'b1;
    $display("paste=%b", value_field);
    $finish;
  end
endmodule
