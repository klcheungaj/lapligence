// An initial procedure blocked on an event that is never triggered can no
// longer complete: the run ends in a deadlock.
module tb;
  event ev;
  initial begin
    @(ev);
    $display("never printed");
  end
  initial #1 $display("other initial done");
  final $display("final");
endmodule
