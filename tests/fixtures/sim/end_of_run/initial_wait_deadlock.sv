// A wait on a condition nobody changes blocks the initial procedure for
// good, even while permanent processes exist.
module tb;
  logic done = 1'b0, copy;
  always @(done) copy = done;
  initial begin
    wait (done);
    $display("never printed");
  end
  final $display("final copy=%b", copy);
endmodule
