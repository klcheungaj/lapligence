// Decision A9-D2: $removal(reference_event, data_event, limit) takes the
// timestamp at the data event and checks at the reference event, in both
// editions: a reference event within the limit after the data event
// violates.
//
// IEEE 1800-2009 31.3.4 Table 31-4 (SystemVerilog-1800-2009.txt
// L51182-51183): "reference_event Timecheck event", "data_event Timestamp
// event"; L51200-51202: "(beginning of time window) < (timestamp time) < (end
// of time window)". IEEE 1364-2001 Table 52 (Verilog-1364-2001.txt
// L16090-16092) lists the roles the other way round although its window
// (L16100-16102) is the same; llg follows the 1800-2009 table.
`timescale 1ns/1ns
module rm(input r, input c); reg n; specify $removal(posedge r, posedge c, 10, n); endspecify endmodule
module tb;
  reg c;
  reg [3:0] r;
  rm m0(r[0], c); rm m1(r[1], c); rm m2(r[2], c); rm m3(r[3], c);
  initial $monitor("%0t removal=%b%b%b%b", $time, m3.n, m2.n, m1.n, m0.n);
  initial begin
    c = 0; r = 0;
    #95 r[0] = 1;                // before the clock: no check
    #5 c = 1;                    // 100: data (timestamp) event
    #1 r[1] = 1;                 // 101: violation
    #8 r[2] = 1;                 // 109: violation
    #1 r[3] = 1;                 // 110: end of the window
    #10 $finish;
  end
endmodule
