// Decision A10-D8: a $nochange window decided at the trailing edge reports
// every data event that was inside it, not only the last one. With the
// negative timing check option a $nochange in a module with delayed
// signals reads the delayed reference; its window is that of the undelayed
// events and is decided at the delayed trailing edge.
//
// IEEE 1800-2009 31.4.6 (SystemVerilog-1800-2009.txt L51960-51962): "The
// $nochange timing check reports a timing violation in the following case:
// (beginning of time window) < (data event time) < (end of time window)";
// 31.9.1 L52432-52436: "The delayed versions of the signals, whether
// implicit or explicit, shall be used in the $setup, $hold, $setuphold,
// $recovery, $removal, $recrem, $width, $period, and $nochange timing
// checks".
`timescale 1ns/1ns
module na(input c, input d); reg n; specify $nochange(negedge c, d, -2, -3, n); endspecify endmodule
// C is delayed by 6 (1 - (-5)); E is not delayed.
module ng(input C, input F, input E); reg nf, n; wire dC, dF;
  specify
    $setuphold(posedge C, F, -5, 10, nf, , , dC, dF);
    $nochange(posedge C, E, 0, 0, n);
  endspecify
endmodule
module tb;
  reg c, gc, gf;
  reg [2:0] d, ge;
  na a0(c, d[0]);   // window (102, 117)
  ng g0(gc, gf, ge[0]);
  ng g1(gc, gf, ge[1]);
  ng g2(gc, gf, ge[2]);
  initial $monitor("%0t a=%b g=%b%b%b", $time, a0.n, g2.n, g1.n, g0.n);
  initial begin
    c = 1; d = 0;
    #100 c = 0;
    #5 d[0] = 1;          // 105: inside
    #13 d[0] = 0;         // 118: outside; a0 still violates at 120
    #2 c = 1;
  end
  initial begin
    gc = 0; gf = 0; ge = 0;
    #200 gc = 1;          // window (200, 220), decided at 226
    #3 ge[0] = 1;         // 203: g0 at 226
    #16 ge[2] = 1;        // 219: g2 at 226
    #1 gc = 0;
    #3 ge[1] = 1;         // 223: none
    #20 $finish;
  end
endmodule
