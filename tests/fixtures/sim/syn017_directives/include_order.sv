// The caller directory must take precedence over a later -I root.
`include "choice.svh"
module tb;
  initial begin
    $display("choice=%0d", `SYN017_CHOICE);
    $finish;
  end
endmodule
