// SIM-025 A03: postponed callbacks do not change design state (SV 4.4.2.9).
// The argument helpers of $strobe and $monitor may keep state of their own,
// but a report leaves every design variable, and the time slot, unchanged:
// the process that follows the report sees exactly the settled values.
module tb;
  reg [7:0] a = 8'd1, b = 8'd0, snap = 8'd0;

  function [7:0] tag(input [7:0] v);
    static reg [7:0] calls = 8'd0;
    calls = calls + 8'd1;
    tag = v;
  endfunction

  initial b <= 8'd3;

  initial begin
    $strobe("S a=%0d t=%0d", tag(a), $time);
    $monitor("M a=%0d b=%0d", tag(a), b);
    a = 8'd2;
    #1;
    snap = a + b;
    $display("D snap=%0d a=%0d b=%0d t=%0d", snap, a, b, $time);
    a = 8'd5;
    #1;
    $display("D a=%0d b=%0d t=%0d", a, b, $time);
    $finish(0);
  end
endmodule
