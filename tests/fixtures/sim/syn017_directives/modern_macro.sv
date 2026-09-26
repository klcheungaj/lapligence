// IEEE 1800-2009 22.5.1: token paste and macro quote construct a
// declaration name and a string from macro arguments.
`define SYN017_CAT(a,b) a``b
`define SYN017_STR(x) `"x`"
module tb;
  reg [6:0] `SYN017_CAT(value,_field);
  initial begin
    `SYN017_CAT(value,_field) = 7'd37;
    $display("name=%s value=%0d", `SYN017_STR(value_field),
             `SYN017_CAT(value,_field));
    $finish;
  end
endmodule
