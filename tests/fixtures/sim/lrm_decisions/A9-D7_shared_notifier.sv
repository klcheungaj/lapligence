// Decision A9-D7: when several checks with one notifier violate at the same
// event, the notifier changes once.
//
// IEEE 1800-2009 31.6 (SystemVerilog-1800-2009.txt L52040-52041): "Whenever
// a timing violation occurs, the timing check updates the value of the
// notifier." 31.8 (L52273-52275): with per-bit checks "all the timing checks
// trigger that notifier". A single update per event is industry-standard
// behaviour.
`timescale 1ns/1ns
module sh(input d, input e, input c); reg n;
  specify $setup(d, posedge c, 10, n); $setup(e, posedge c, 10, n); endspecify
endmodule
module tb;
  reg c, d, e;
  sh s(d, e, c);
  initial $monitor("%0t n=%b", $time, s.n);
  initial begin
    c = 0; d = 0; e = 0;
    #95 d = 1; e = 1;
    #5 c = 1;                    // 100: both checks violate: x -> 0
    #10 c = 0;
    #5 d = 0; e = 0;
    #5 c = 1;                    // 120: both again: 0 -> 1
    #10 $finish;
  end
endmodule
