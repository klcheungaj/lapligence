// A do-loop jump bypasses the write to persistent function-result storage.
module tb;
  bit toggle;
  integer changes;
  function bit [7:0] retained(input bit write_it);
    do begin
      if (!write_it) break;
      retained = 8'd7;
    end while (0);
  endfunction
  always @(retained(toggle)) changes = changes + 1;
  initial begin
    changes = 0;
    #1 toggle = 1;
    #1 toggle = 0;
    #1 $display("PERSISTENT_RESULT changes=%0d", changes);
    $finish(0);
  end
endmodule
