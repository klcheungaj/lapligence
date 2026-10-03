// Persistent static return mutation is legal (SV 13.4.2); the waiting process evaluates it.
module tb;
  bit toggle;
  integer changes;
  function bit [7:0] accumulator(input bit increment);
    if (increment) accumulator++;
  endfunction
  always @(accumulator(toggle)) changes = changes + 1;
  initial begin
    changes = 0;
    toggle = 0;
    #1 toggle = 1;
    #1 $display("ACCEPTED_STATEFUL_CALLBACK changes=%0d", changes);
    $finish;
  end
endmodule
