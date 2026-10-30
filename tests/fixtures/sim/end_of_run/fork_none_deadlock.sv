// The parent finishes; the join_none child is still blocked on an event
// that never comes, which is a deadlock.
module tb;
  event ev;
  initial begin
    fork
      @(ev);
    join_none
    #1 $display("parent done");
  end
  final $display("final");
endmodule
