// ADV-026 step 3: a $sreadmemh into a watched memory notifies exactly like a
// $readmemh of the same image (IEEE 1800-2009 4.3: "Every change in state of
// a net or variable ... is considered an update event"; 21.4: "each number
// encountered is assigned to a successive word element of the memory", and
// the strings "take the same format as the strings that appear in the input
// files" in D.14). Memory `mf` is loaded from a file, `ms` from strings of the
// same words; both are watched by processes (`w<i>`), by continuous
// assignments (`sum`, `tot`) and by VPI value-change callbacks (notify.c,
// which prints only while `armed`). The words of one time step may print in
// any order (4.7); the test compares them as sets.
`timescale 1ns / 1ns

module tb;
  logic [7:0] mf [0:3];
  logic [7:0] ms [0:3];
  logic [7:0] sumf, sums;
  logic [8:0] totf, tots;
  logic armed;
  assign sumf = mf[1];
  assign sums = ms[1];
  assign totf = mf[0] + mf[2];
  assign tots = ms[0] + ms[2];

  always @(mf[0]) if (armed) $display("[%0t] f w0 %h", $time, mf[0]);
  always @(mf[1]) if (armed) $display("[%0t] f w1 %h", $time, mf[1]);
  always @(mf[2]) if (armed) $display("[%0t] f w2 %h", $time, mf[2]);
  always @(mf[3]) if (armed) $display("[%0t] f w3 %h", $time, mf[3]);
  always @(ms[0]) if (armed) $display("[%0t] s w0 %h", $time, ms[0]);
  always @(ms[1]) if (armed) $display("[%0t] s w1 %h", $time, ms[1]);
  always @(ms[2]) if (armed) $display("[%0t] s w2 %h", $time, ms[2]);
  always @(ms[3]) if (armed) $display("[%0t] s w3 %h", $time, ms[3]);

  initial begin
    armed = 0;
    mf[0] = 8'h00; mf[1] = 8'h22; mf[2] = 8'h00; mf[3] = 8'h77;
    ms[0] = 8'h00; ms[1] = 8'h22; ms[2] = 8'h00; ms[3] = 8'h77;
    #1 armed = 1;
    // Words 0 and 2 change; word 1 is loaded with its current value; word 3
    // is outside the range.
    #1 $readmemh("w1.hex", mf, 0, 2);
    #1 $sreadmemh(ms, 0, 2, "11", "22 33");
    // The same image again changes nothing.
    #1 $readmemh("w1.hex", mf, 0, 2);
    #1 $sreadmemh(ms, 0, 2, "11", "22 33");
    // Only word 1 differs.
    #1 $readmemh("w2.hex", mf, 0, 2);
    #1 $sreadmemh(ms, 0, 2, "11 99", "33");
    #1 $display("[%0t] final f %h %h %h %h", $time, mf[0], mf[1], mf[2], mf[3]);
    $display("[%0t] final s %h %h %h %h", $time, ms[0], ms[1], ms[2], ms[3]);
    $finish;
  end
endmodule
