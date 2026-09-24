// No explicit return read, but the static result persists on an unassigned path.
module tb;
  bit toggle;
  integer changes;
  function bit [7:0] retained(input bit write_it);
    if (write_it) retained = 8'd7;
  endfunction
  always @(retained(toggle)) changes = changes + 1;
  initial begin
    changes = 0;
    toggle = 0;
    #1 toggle = 1;
    #1 toggle = 0;
    #1 $display("ACCEPTED_PERSISTENT_RESULT changes=%0d", changes);
    $finish;
  end
endmodule
