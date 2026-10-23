// Decision A10-D3: with the negative timing check option every signal of a
// negative $setuphold or $recrem gets one delayed copy per edge class (rise:
// 01, 0x, x1; fall: 10, 1x, x0), with the smallest delays that put the
// reference event at least one precision unit inside a violation window of
// two or more units: d_ref - d_data >= 1 - setup and d_data - d_ref >= 1 -
// hold (0 - limit for a zero limit). A class that no check constrains takes
// the delay of the other class. A delayed signal starts x and takes its
// signal's initial value after the delay of that transition.
//
// IEEE 1800-2009 31.9.1 (SystemVerilog-1800-2009.txt L52377-52379): "A
// timing violation shall be triggered if the signal changes in the violation
// window, exclusive of the end points. Violation windows smaller than two
// units of simulation precision cannot yield timing violations."; L52460-52466:
// "$setuphold (posedge CLK, posedge D, -3, 8, , , , dCLK, dD); $setuphold
// (posedge CLK, negedge D, -7, 13, , , , dCLK, dD); The setup time of -7 (the
// larger in absolute value of -3 and -7) creates a delay of 7 for dCLK".
`timescale 1ns/1ns
module pe(input C, input D); reg n; wire dC, dD;
  specify
    $setuphold(posedge C, posedge D, -3, 8, n, , , dC, dD);
    $setuphold(posedge C, negedge D, -7, 13, n, , , dC, dD);
  endspecify
endmodule
module pd(input C, input D); reg n; wire dC, dD;
  specify $setuphold(posedge C, posedge D, 8, -2, n, , , dC, dD); endspecify
endmodule
module tb;
  reg c, d;
  pe a(c, d);
  pd b(c, d);
  always @(a.dC) if ($time > 0) $display("%0t pe dC=%b", $time, a.dC);
  always @(a.dD) if ($time > 0) $display("%0t pe dD=%b", $time, a.dD);
  always @(b.dC) if ($time > 0) $display("%0t pd dC=%b", $time, b.dC);
  always @(b.dD) if ($time > 0) $display("%0t pd dD=%b", $time, b.dD);
  initial begin
    c = 0; d = 0;
    // Initial values: pe dC at 8, pd dD at 3 (x0 is a falling transition).
    #100 c = 1;  // pe: dC 108 (8 = 1 - (-7)); pd: dC 100
    #20 c = 0;   // pe: dC 128 (the falling class takes 8); pd: dC 120
    #20 d = 1;   // pe: dD 141 (8 - 7 for the -7/13 hold); pd: dD 143 (1 - (-2))
    #10 d = 0;   // pe: dD 150; pd: dD 153 (the falling class takes 3)
    #20 $display("%0t notifiers %b %b", $time, a.n, b.n);
    $finish;
  end
endmodule
