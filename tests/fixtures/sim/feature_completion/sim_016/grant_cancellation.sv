// SIM-016: a put takes the keys it grants to a waiter, so a waiter killed
// after the grant but before it resumes keeps them (decision AB-N4); a
// process killed while holding keys keeps them taken (SV 15.3, 9.6.3).
module tb;
  semaphore s = new(0);
  initial begin
    fork begin s.get(1); $display("FAIL killed waiter ran"); end join_none
    fork begin #1 s.get(1); $display("holder got %0d", $time); #10; end join_none
    #3 s.put(1);
    disable fork;
    if (s.try_get(1)) $display("FAIL granted key returned"); else $display("granted key kept %0d", $time);
    s.put(1);
    fork begin s.get(1); $display("holder2 got %0d", $time); #10; end join_none
    #1 disable fork;
    if (s.try_get(1)) $display("FAIL holder key duplicated"); else $display("held key kept %0d", $time);
  end
endmodule
