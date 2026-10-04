// $strobe/$monitor arguments whose helpers keep their own static state or
// take descriptor-transported arrays (SV 4.4.2.9, 13.4.2, 21.2.2, 21.2.3).
`timescale 1ns / 1ns
module tb;
  int a = 1;
  int big [0:65536];

  function automatic int ends(input int v [0:65536]);
    return v[0] + v[65536];
  endfunction

  // Static local: counts its own evaluations; the result does not depend on
  // the count.
  function int tens(input int x);
    static int calls = 0;
    calls++;
    return x * 10;
  endfunction

  // Persistent static result: accumulates across calls.
  function int acc(input int x);
    acc = acc + x;
  endfunction

  initial begin
    big[0] = 5;
    big[65536] = 6;
    $strobe("s0 %0d %0d %0d", ends(big), tens(a), acc(a));
    #1 a = 2;
    big[0] = 7;
    $strobe("s1 %0d %0d %0d", ends(big), tens(a), acc(a));
    #1 $monitor("m %0d %0d", tens(a), ends(big));
    #1 a = 3;
    #1 a = 3;
    #1 big[65536] = 0;
    #1 $display("calls_ok=%0d", tens.calls >= 4);
    $finish(0);
  end
endmodule
