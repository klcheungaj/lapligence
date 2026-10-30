// A waiter that is woken before the events run out does not deadlock; the
// permanent processes alone then leave the normal end message.
module tb;
  event ev;
  logic a = 1'b1;
  wire b = a;
  initial #1 -> ev;
  initial begin
    @(ev);
    $display("woken b=%b", b);
  end
  initial #5 $display("late initial done");
endmodule
