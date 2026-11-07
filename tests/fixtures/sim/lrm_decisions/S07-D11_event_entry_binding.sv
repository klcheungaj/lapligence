// Decision S07-D11: an event control, a trigger and `wait (e.triggered)` on a
// missing entry of an associative array of events create the entry with a new
// event, so a later wait or trigger of that key names the same event. A read
// of the element (`.triggered` as a value, an assignment source) creates
// nothing.
//
// IEEE 1800-2009 7.8.6 (SystemVerilog-1800-2009.txt L8720, L8735) covers the
// value a read of a nonexistent entry returns ("be returned, as shown in Table
// 7-1"); the text is silent on an event control or trigger of one. llg follows
// industry-standard behaviour.
module tb;
  event ev[int];
  event es[string];
  event alias1;
  bit ok;
  initial begin
    fork
      begin @(ev[3]); $display("woke3 %0d", $time); end
      begin @(es["a"]); $display("wokea %0d", $time); end
      begin wait (ev[5].triggered); $display("woke5 %0d", $time); end
    join_none
    #1 $display("%0d %0d", ev.num(), es.num());
    -> ev[3];
    -> es["a"];
    -> ev[5];
    #1 -> ev[4];
    $display("%0d %0d", ev.num(), ev.exists(4));
    fork
      begin @(ev[4]); $display("woke4 %0d", $time); end
    join_none
    #1 -> ev[4];
    #1 ok = ev[6].triggered;
    $display("%0d %0d", ok, ev.num());
    alias1 = ev[7];
    $display("%0d", ev.num());
  end
endmodule
