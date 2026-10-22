// `$width` with a notifier needs a threshold (31.4.4: "$width ( negedge clr,
// lim, , notif );" is an illegal call).
`timescale 1ns/1ns
module m(input c);
  reg n;
  specify
    $width(negedge c, 10, , n);
  endspecify
endmodule
module tb;
  reg c;
  m u(c);
  initial begin $display("ran"); #1 $finish; end
endmodule
