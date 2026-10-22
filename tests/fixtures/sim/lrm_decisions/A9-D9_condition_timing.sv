// Decision A9-D9: a condition is read when its event occurs, after the
// statements of that time step have settled it; a false condition on the
// timestamp event records no timestamp. $width conditions gate both edges;
// the $nochange edges always open and close the window and its conditions
// are read at the data event. `==` and `!=` with x enable; `===`, `!==`,
// `~` and a plain expression with x disable.
//
// IEEE 1800-2009 31.7 (SystemVerilog-1800-2009.txt L52190-52194): "The
// comparisons used in the condition can be deterministic, as in ===, !==, ~,
// or no operation, or nondeterministic, as in == or !=. When comparisons are
// deterministic, an x value on the conditioning signal shall not enable the
// timing check. For nondeterministic comparisons, an x on the conditioning
// signal shall enable the timing check."
`timescale 1ns/1ns
module k0(input d, input c, input en); reg n; specify $setup(d, posedge c &&& en, 10, n); endspecify endmodule
module k1(input d, input c, input en); reg n; specify $setup(d, posedge c &&& (en == 1), 10, n); endspecify endmodule
module k2(input d, input c, input en); reg n; specify $setup(d, posedge c &&& (en !== 0), 10, n); endspecify endmodule
module ts(input d, input c, input en); reg n; specify $setup(d &&& en, posedge c, 10, n); endspecify endmodule
module tw(input c, input en); reg n; specify $width(posedge c &&& en, 10, 0, n); endspecify endmodule
module tn(input c, input d, input en); reg n; specify $nochange(posedge c &&& en, d, 0, 0, n); endspecify endmodule
module tb;
  reg c, d, x;
  reg [1:0] sd, sen, wc, wen, nd, nen;
  k0 a(d, c, x); k1 b(d, c, x); k2 e(d, c, x);
  ts s0(sd[0], c, sen[0]); ts s1(sd[1], c, sen[1]);
  tw w0(wc[0], wen[0]); tw w1(wc[1], wen[1]);
  tn q0(c, nd[0], nen[0]); tn q1(c, nd[1], nen[1]);
  initial
    $monitor("%0t x: plain=%b eq=%b nce=%b timestamp=%b%b width=%b%b nochange=%b%b", $time,
             a.n, b.n, e.n, s1.n, s0.n, w1.n, w0.n, q1.n, q0.n);
  initial begin
    c = 0; d = 0; x = 1'bx; sd = 0; sen = 2'b01; wc = 0; wen = 2'b11; nd = 0; nen = 2'b10;
    #95 d = 1; sd = 2'b11;
    sen[0] = 0; sen[1] = 1;      // 95: timestamp conditions settle to 0 and 1
    #5 c = 1; wc = 2'b11;        // 100
    nen[0] = 1; nen[1] = 0;      // nochange conditions at the data event: 1, 0
    #2 wen[1] = 0;               // width condition of w1 false at its falling edge
    #1 nd = 2'b11;               // 103: data while c is high
    #2 wc = 0;                   // 105: width 5
    #10 $finish;
  end
endmodule
