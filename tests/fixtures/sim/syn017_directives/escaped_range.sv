// IEEE 1364-2001 2.7.1 and 19.3.1: an escaped identifier and an
// expanded range survive elaboration into a runtime value.
`define SYN017_RANGE(hi,lo) [hi:lo]
module tb;
  reg `SYN017_RANGE(6,0) \value.with.dot ;
  initial begin
    \value.with.dot  = 7'd65;
    $display("escaped=%b", \value.with.dot );
    $finish;
  end
endmodule
