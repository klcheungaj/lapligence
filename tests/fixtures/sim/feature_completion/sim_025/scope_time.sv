// SIM-025 A01: %m, %t and $time in monitored lists (SV 21.2.1.6, 21.2.3).
// $time never triggers a report; two $fmonitor lists registered by different
// instances stay active together and each names its own instance.
`timescale 1ns/1ps
module sub #(parameter ID = 0, parameter T = 10, parameter D = 0);
  reg [1:0] v = 0;
  initial begin
    #D $fmonitor(1, "%m: t=%0t time=%0d v=%0d", $time, $time, v);
    #T v = ID;
    #T v = ID;
    #T v = 3;
  end
endmodule

module tb;
  sub #(1, 10, 0) s1();
  sub #(2, 15, 5) s2();
  initial begin
    $timeformat(-9, 0, " ns", 1);
    #60 $finish(0);
  end
endmodule
