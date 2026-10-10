// SIM-025 A02: replacing a $monitor, $monitoroff/$monitoron and a $monitor
// issued while the flag is off (SV 21.2.3). Exactly one list is active, a
// replaced list leaves no registration behind, and the monitor flag outlives
// the list it was set under.
module tb;
  reg [3:0] a = 1, b = 2, c = 3;
  initial begin
    $monitor("A a=%0d", a);
    #1 a = 2;
    #1 $monitor("B b=%0d", b);
    a = 3;
    #1 a = 4; c = 9;
    #1 b = 5;
    #1 $monitoroff;
    b = 6;
    #1 b = 7;
    #1 $monitoron;
    #1 $monitoron;
    #1 $monitoroff;
    $monitor("C c=%0d", c);
    #1 c = 1;
    #1 $monitoron;
    #1 b = 8; c = 2; a = 9;
    #1 $finish(0);
  end
endmodule
