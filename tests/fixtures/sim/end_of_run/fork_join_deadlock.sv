// A fork whose child is blocked forever keeps its parent blocked at the
// join; the parent and the child are both initial-level waiters.
module tb;
  event ev;
  initial begin
    fork
      @(ev);
    join
    $display("never printed");
  end
  final $display("final");
endmodule
