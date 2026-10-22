// Decision A9-D4: $width reports a pulse only when it is wider than the
// threshold and narrower than the limit.
//
// IEEE 1800-2009 31.4.4 (SystemVerilog-1800-2009.txt L51818-51824): "The
// $width timing check reports a violation in the following case: threshold <
// (timecheck time) - (timestamp time) < limit ... no violation is reported
// for glitches smaller than the threshold."
`timescale 1ns/1ns
module wi(input c); reg n; specify $width(posedge c, 10, 3, n); endspecify endmodule
module tb;
  reg [3:0] c;
  wi w0(c[0]); wi w1(c[1]); wi w2(c[2]); wi w3(c[3]);
  initial $monitor("%0t width=%b%b%b%b", $time, w3.n, w2.n, w1.n, w0.n);
  initial begin
    c = 0;
    #100 c = 4'hf;
    #2 c[0] = 0;                 // 102: width 2, below the threshold
    #1 c[1] = 0;                 // 103: width 3, the threshold
    #1 c[2] = 0;                 // 104: width 4
    #6 c[3] = 0;                 // 110: width 10, the limit
    #10 $finish;
  end
endmodule
