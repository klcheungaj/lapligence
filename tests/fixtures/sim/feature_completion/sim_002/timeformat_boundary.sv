// SIM-002: time reporting at time 0 and at the last representable tick
// (2^64-1 fs). %t converts exactly in integer arithmetic, including a 1s
// module value whose femtosecond image exceeds 64 bits; $stime keeps the low
// 32 bits; $time rounds halves upward. SV2009 20.3.1-20.3.2, 20.4.2, 21.2.1.3.
`timescale 1s/1fs
module sec;
  task automatic show();
    $display("sec time=%0d stime=%0d t=%t", $time, $stime, $time);
  endtask
endmodule
`timescale 1fs/1fs
module tb;
  sec s();
  initial begin
    $display("zero [%t] [%0t]", $time, $realtime);
    $timeformat(0, 3, " s", 0);
    $display("zero seconds [%t]", $time);
    $timeformat;
    #(64'hFFFF_FFFF_FFFF_FFFF);
    $display("last time=%0d stime=%0d t=%t", $time, $stime, $time);
    $timeformat(0, 15, " s", 0);
    $display("seconds [%t]", $time);
    $timeformat(0, 0, " s", 25);
    $display("whole seconds [%t]", $time);
    $timeformat(-3, 0, " ms", 0);
    $display("milli [%t]", $time);
    $timeformat(-15, 0, "", 0);
    s.show();
    $timeformat(-15, 2, "", 30);
    s.show();
    $finish(0);
  end
endmodule
