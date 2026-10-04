// RTL-012: a strength-only change (Pu1 <-> St1) reaches `%v` monitors but not
// value waiters; an unchanged resolution wakes nobody (IEEE 1364-2001 7.10,
// 17.1.1.5; IEEE 1800-2009 4.4, 21.2.3). Hand-derived output.
module tb;
  reg s, k;
  wire w;
  pullup (w);
  assign w = s ? 1'b1 : 1'bz;
  assign w = k ? 1'b0 : 1'bz;
  wire [1:0] bus;
  pulldown (bus[1]);
  bufif1 (bus[1], s, k);
  integer wakes = 0;
  integer bus_wakes = 0;
  always @(w) if ($time > 0) wakes = wakes + 1;
  always @(bus) if ($time > 0) bus_wakes = bus_wakes + 1;
  initial begin
    $monitor("%0d %v %b %v", $time, w, w, bus[1]);
    s = 0;
    k = 0;
    #1 s = 1;
    #1 s = 0;
    #1 k = 1;
    #1 k = 1;
    #1 $display("wakes %0d %0d", wakes, bus_wakes);
    $finish(0);
  end
endmodule
