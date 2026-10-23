// Decision A10-D9: the $timeskew and $fullskew event_based_flag and
// remain_active_flag are constant expressions set when nonzero (an unknown
// value is an error). A timer-based check reports at the expiry of the
// limit, an event-based one at the late data event, and with
// remain_active_flag at every later one.
//
// IEEE 1800-2009 31.4.2 (SystemVerilog-1800-2009.txt L51512-51522): "The
// default behavior for $timeskew is timer-based. A violation shall be
// reported immediately upon an elapse of time after the reference event
// equal to the limit, and the check shall become dormant and report no more
// violations (even in response to data events) until after the next
// reference event. ... It behaves like the $skew check when both the
// event_based_flag and the remain_active_flag are set."; 31.4.3
// L51655-51657: "limit is set to limit1 when the reference event transitions
// first and set to limit2 when the data event transitions first".
`timescale 1ns/1ns
module t00(input a, input b); reg n; specify $timeskew(posedge a, negedge b, 10, n); endspecify endmodule
module t10(input a, input b); reg n; specify $timeskew(posedge a, negedge b, 10, n, 1); endspecify endmodule
module t11(input a, input b); reg n; specify $timeskew(posedge a, negedge b, 10, n, 2'b10, 1); endspecify endmodule
module f00(input a, input b); reg n; specify $fullskew(posedge a, negedge b, 10, 20, n); endspecify endmodule
module tb;
  reg a, b, fa, fb;
  t00 x(a, b);
  t10 y(a, b);
  t11 z(a, b);
  f00 f(fa, fb);
  initial $monitor("%0t timer=%b event=%b remain=%b full=%b", $time, x.n, y.n, z.n, f.n);
  initial begin
    a = 0; b = 1;
    #100 a = 1;           // the window ends at 110: timer reports at 110
    #15 b = 0;            // 115: event-based checks report
    #5 b = 1;
    #5 b = 0;             // 125: only remain_active reports again
  end
  initial begin
    fa = 0; fb = 1;
    #200 fb = 0;          // data first: limit 2 = 20
    #20 fa = 1;           // 220: the limit itself: none
    #5 fa = 0;
    #5 fb = 1;
    #5 fb = 0;            // 235: data stamps again
    #21 fa = 1;           // 256: the timer reported at 255; a new window
    #2 fb = 1;
    #2 fb = 0;            // 260: within limit 1 = 10: none
    #20 $finish;
  end
endmodule
