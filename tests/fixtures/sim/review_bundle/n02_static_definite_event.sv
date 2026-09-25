// A whole overwrite before any jumps is safe; inner breaks stay in their loop.
module tb;
  bit toggle;
  integer changes;
  function bit definite(input bit value);
    definite = value;
    do begin
      if (value) continue;
      repeat (1) begin
        break;
      end
    end while (0);
  endfunction
  always @(definite(toggle)) changes = changes + 1;
  initial begin
    changes = 0;
    #1 toggle = 1;
    #1 toggle = 0;
    #1;
    if (changes !== 2) $fatal(1, "eligible static helper event transitions");
    $display("PASS n02_static_definite_event");
    $finish(0);
  end
endmodule
