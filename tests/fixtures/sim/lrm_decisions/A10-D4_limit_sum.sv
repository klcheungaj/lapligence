// Decision A10-D4: with the negative timing check option a check whose
// setup and hold (removal and recovery) limits sum below 0 has its negative
// limit set to 0 (llg warns); a sum of 0 or 1 keeps the limits for the
// delayed signals (llg warns) and the check never reports, its window being
// shorter than two units, also for data on the delayed reference event.
//
// IEEE 1800-2009 31.3.3 (SystemVerilog-1800-2009.txt L51110-51112): "When
// either the setup limit or the hold limit is negative, the restriction
// becomes as follows: setup_limit + hold_limit > (simulation unit of
// precision)"; 31.9.1 L52378-52379: "Violation windows smaller than two units
// of simulation precision cannot yield timing violations."
`timescale 1ns/1ns
module ne(input r, input d); reg n; specify $setuphold(posedge r, d, -5, 4, n); endspecify endmodule
module nf(input r, input d); reg n; specify $setuphold(posedge r, d, -5, 5, n); endspecify endmodule
module ng(input r, input d); reg n; specify $setuphold(posedge r, d, -5, 6, n); endspecify endmodule
module tb;
  reg r;
  reg [6:0] d;
  ne e0(r, d[0]);  // sum -1: setup 0, hold [100, 104)
  ne e1(r, d[1]);
  nf f0(r, d[2]);  // sum 0: no window
  nf f1(r, d[3]);
  ng g0(r, d[4]);  // sum 1: no window
  ng g1(r, d[5]);
  ng g2(r, d[6]);
  initial $monitor("%0t e=%b%b f=%b%b g=%b%b%b", $time, e1.n, e0.n, f1.n, f0.n, g2.n, g1.n, g0.n);
  initial begin
    r = 0; d = 0;
    #100 r = 1;
    #3 d[0] = 1;                  // 103: hold of e0
    #1 d[1] = 1;                  // 104: none
    d[2] = 1; d[4] = 1;           // 104
    #1 d[3] = 1; d[5] = 1;        // 105: the delayed reference time: none
    #1 d[6] = 1;                  // 106
    #10 $finish;
  end
endmodule
