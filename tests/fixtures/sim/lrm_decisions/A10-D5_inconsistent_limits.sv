// Decision A10-D5: when the negative limits of a set of checks admit no
// delays, llg warns, sets the smallest (most negative) limit among the
// checks that conflict to 0 and solves again, until a solution is found.
//
// IEEE 1800-2009 31.9.1 (SystemVerilog-1800-2009.txt L52444-52449): "It is
// possible for a set of negative timing check values to be mutually
// inconsistent and produce no solution for the delay values of delayed
// signals. In these situations, the simulator shall issue a warning. The
// inconsistency shall be resolved by changing the smallest negative limit
// value to 0 and recalculating the delays for the delayed signals, and this
// shall be repeated until a solution is reached."
`timescale 1ns/1ns
module cyc(input A, input B); reg n; wire dA, dB;
  specify
    $setuphold(posedge A, B, -5, 10, n, , , dA, dB);  // needs dA > dB
    $setuphold(posedge B, A, -6, 10, n, , , dB, dA);  // needs dB > dA
  endspecify
endmodule
module tb;
  reg a, b;
  cyc u(a, b);
  // -6 and then -5 become 0: no delays; B at 106 is in the hold window
  // [100, 110) of the first check.
  initial $monitor("%0t a=%b dA=%b b=%b dB=%b n=%b", $time, a, u.dA, b, u.dB, u.n);
  initial begin
    a = 0; b = 0;
    #100 a = 1;
    #6 b = 1;
    #20 $finish;
  end
endmodule
