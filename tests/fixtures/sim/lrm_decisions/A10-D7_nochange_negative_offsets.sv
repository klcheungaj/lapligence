// Decision A10-D7: negative $nochange offsets shrink the window whether or
// not the negative timing check option is given (it applies to $setuphold
// and $recrem). A window that ends before the trailing edge is decided
// there: every data event inside it violates at the trailing edge.
//
// IEEE 1800-2009 31.4.6 (SystemVerilog-1800-2009.txt L51934-51947): "a
// negative offset for start edge shrinks the region by starting the region
// later. ... a negative offset for the end edge shrinks the region by ending
// it earlier."; L51956-51960: "(beginning of time window) = (leading
// reference edge time) - start_edge_offset (end of time window) = (trailing
// reference edge time) + end_edge_offset ... (beginning of time window) <
// (data event time) < (end of time window)".
`timescale 1ns/1ns
module na(input c, input d); reg n; specify $nochange(negedge c, d, -2, -3, n); endspecify endmodule
module ne(input c, input d); reg n; specify $nochange(negedge c, d, -2, 3, n); endspecify endmodule
module tb;
  reg c;
  reg [4:0] d;
  na a0(c, d[0]);   // window (102, 117)
  na a1(c, d[1]);
  ne e0(c, d[2]);   // window (102, 123)
  ne e1(c, d[3]);
  ne e2(c, d[4]);
  initial $monitor("%0t a=%b%b e=%b%b%b", $time, a1.n, a0.n, e2.n, e1.n, e0.n);
  initial begin
    c = 1; d = 0;
    #100 c = 0;           // leading edge
    #1 d[2] = 1;          // 101: none
    #2 d[3] = 1;          // 103: e1 at 103
    #2 d[0] = 1;          // 105: a0, reported at 120
    #12 d[1] = 1;         // 117: none
    #3 c = 1;             // 120: trailing edge
    #2 d[4] = 1;          // 122: e2 at 122
    #10 $finish;
  end
endmodule
