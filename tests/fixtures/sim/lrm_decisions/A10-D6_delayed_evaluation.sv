// Decision A10-D6: with the negative timing check option the timing checks
// of a module read its delayed signals, with each limit adjusted by the
// delays of the two transitions it compares (at least 0); notifiers toggle
// when the delayed signals violate and conditions are read at the delayed
// events. Reports give the undelayed event times.
//
// IEEE 1800-2009 31.9.1 (SystemVerilog-1800-2009.txt L52432-52436): "The
// delayed versions of the signals, whether implicit or explicit, shall be
// used in the $setup, $hold, $setuphold, $recovery, $removal, $recrem,
// $width, $period, and $nochange timing checks; and these checks shall have
// their limits adjusted accordingly so that the notifier shall be toggled at
// the proper moment."; 31.9.2 L52506-52507: "timestamp_condition and
// timecheck_condition are not implicitly delayed by the simulator";
// 31.9.3 L52573-52576: "Notifier variables in negative timing checks shall
// be toggled when the timing check detects a timing violation, which occurs
// when the delayed signals as measured by the adjusted timing check values
// are in violation".
`timescale 1ns/1ns
// Window (103, 108) around r at 100: dr = 4, setup 1 and hold 4 after it.
module na(input r, input d); reg n; wire dr, dd;
  specify $setuphold(posedge r, d, -3, 8, n, , , dr, dd); endspecify
endmodule
// The same check with a timestamp condition, read at the delayed reference.
module nc(input r, input d, input ts); reg n; wire dr, dd;
  specify $setuphold(posedge r, d, -3, 8, n, ts, , dr, dd); endspecify
endmodule
// $hold on the delayed signals of the module's $setuphold: its limit 2 is
// adjusted to 2 + 0 - 4 = 0 for r and d, so it never reports.
module nh(input r, input d); reg n, nh; wire dr, dd;
  specify
    $setuphold(posedge r, d, -3, 8, n, , , dr, dd);
    $hold(posedge r, d, 2, nh);
  endspecify
endmodule
module tb;
  reg r;
  reg [3:0] d;
  reg [1:0] ts;
  na a0(r, d[0]);
  na a1(r, d[1]);
  nc c0(r, d[2], ts[0]);
  nc c1(r, d[2], ts[1]);
  nh h(r, d[3]);
  initial $monitor("%0t a=%b%b c=%b%b h=%b%b", $time, a1.n, a0.n, c1.n, c0.n, h.nh, h.n);
  initial begin
    r = 0; d = 0; ts = 2'b10;
    #100 r = 1;                 // 100 (delayed: 104)
    d[3] = 1;                   // 100: before the window, no $hold report
    #2 ts = 2'b01;              // 102: the conditions settle before 104
    #1 d[0] = 1;                // 103: on the window start: none
    #1 d[1] = 1; d[2] = 1;      // 104: a1 and c0 violate at 104
    #10 $finish;
  end
endmodule
