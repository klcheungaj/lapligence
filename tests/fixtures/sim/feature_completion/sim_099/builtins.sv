// SIM-099 A01: built-in system functions and tasks that no other fixture
// called: $onehot/$onehot0 (IEEE 1800-2009 20.13), $itor (20.5), and the
// binary/octal/hexadecimal file forms of $fdisplay, $fwrite, $fstrobe and
// $fmonitor (21.3.2) writing to the standard-output descriptor
// 32'h8000_0001.
`timescale 1ns / 1ns
module tb;
  localparam integer STDOUT = 32'h8000_0001;
  logic [7:0] x = 8'b0001_0000;
  logic [3:0] y = 4'b0000;
  real r;

  initial begin
    r = $itor(-7);
    $display("onehot %0d %0d %0d onehot0 %0d %0d %0d itor %0.1f", $onehot(x), $onehot(y),
             $onehot(8'b0011_0000), $onehot0(x), $onehot0(y), $onehot0(8'b0011_0000), r);
    $fdisplayb(STDOUT, x);
    $fdisplayo(STDOUT, x);
    $fwriteb(STDOUT, y, "|");
    $fwriteo(STDOUT, x, "|");
    $fwriteh(STDOUT, x, "\n");
    $fstrobeh(STDOUT, "strobe h ", x);
    $fstrobeo(STDOUT, "strobe o ", x);
    x = 8'hff;
    #1;
    $fmonitorb(STDOUT, "monitor b ", y);
    #1 y = 4'b1010;
    #1;
    $fmonitoro(STDOUT, "monitor o ", x);
    #1 x = 8'o17;
    #1 $finish;
  end
endmodule
