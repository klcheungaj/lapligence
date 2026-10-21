// Decision T0-D4: a constant driver's z bits give the net no driven value, so
// those net bits keep their default z and the driver's first evaluation at
// time 0 is no change there; an `always @(w)` that is already waiting (T0-D2)
// does not wake for them. A driver that computes z, and a constant driver's
// 0 and 1 bits, start the driven bits at X (T0-D3) and are a time-0 change.
//
// IEEE 1800-2009 6.7 (SystemVerilog-1800-2009.txt L5199-5200):
//   "The default initialization value for a net shall be the value z. Nets
//   with drivers shall assume the output value of their drivers."
// IEEE 1800-2009 9.4.2 (L11767-11768):
//   "An implicit event shall be detected on any change in the value of the
//   expression."
module tb;
  logic en = 1'b0;
  logic d = 1'b1;
  wire wz;
  wire [3:0] wf;
  wire [3:0] wd = 4'bzzzz;
  wire [3:0] wp;
  wire wt;
  wire w2;
  assign wz = 1'bz;
  assign wf = 'z;
  assign wp = {2'b1z, 2'bzz};
  assign wt = en ? d : 1'bz;
  assign w2 = 1'bz;
  assign w2 = 1'b0;
  int n_wz, n_wf, n_wd, n_wp, n_wt, n_w2;
  always @(wz) n_wz++;
  always @(wf) n_wf++;
  always @(wd) n_wd++;
  always @(wp) n_wp++;
  always @(wt) n_wt++;
  always @(w2) n_w2++;
  initial begin
    #1 $display("t1 wz=%b wf=%b wd=%b wp=%b wt=%b w2=%b", wz, wf, wd, wp, wt, w2);
    $display("time-0 events: wz=%0d wf=%0d wd=%0d wp=%0d wt=%0d w2=%0d",
             n_wz, n_wf, n_wd, n_wp, n_wt, n_w2);
    en = 1'b1;
    #1 en = 1'b0;
    #1 $display("t3 wt=%b events wt=%0d", wt, n_wt);
    $finish;
  end
endmodule
