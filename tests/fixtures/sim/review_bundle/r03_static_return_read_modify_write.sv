// The result is initialized on every path but read by the increment, so it is
// not a read-only callback; the waiting process evaluates it (SV 9.4.2).
module tb;
  bit toggle;
  integer changes;
  function bit [7:0] accumulator(input bit increment);
    accumulator = 8'd3;
    if (increment) accumulator++;
  endfunction
  always @(accumulator(toggle)) changes = changes + 1;
  initial begin
    changes = 0;
    toggle = 0;
    #1 toggle = 1;
    #1 $display("ACCEPTED_READ_MODIFY_WRITE changes=%0d", changes);
    $finish;
  end
endmodule
