// Decision A9-D3: transitions at time 0 are neither timestamp nor timecheck
// events; the initial values settle without violations.
//
// IEEE 1800-2009 31.3.1 (SystemVerilog-1800-2009.txt L50983-50985) defines
// violations by the times of the two events only and says nothing about
// time 0; ignoring time-0 transitions is industry-standard behaviour.
`timescale 1ns/1ns
module su(input d, input c); reg n; specify $setup(d, posedge c, 10, n); endspecify endmodule
module ho(input d, input c); reg n; specify $hold(posedge c, d, 10, n); endspecify endmodule
module tb;
  reg c, d, c2, d2;
  su s(d, c); ho h(d2, c2);
  initial $monitor("%0t setup=%b hold=%b", $time, s.n, h.n);
  initial begin
    d = 1; c = 0; c2 = 1; d2 = 0;  // time 0: data and clock change
    #5 c = 1; d2 = 1;              // 5: within both limits of time 0
    #20 d = 0;
    #5 c = 0;
    #1 c = 1;                      // 31: setup 6 < 10
    #10 $finish;
  end
endmodule
