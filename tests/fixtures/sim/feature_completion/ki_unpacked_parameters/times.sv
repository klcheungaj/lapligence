module tb;
  timeunit 1ns; timeprecision 1ps;
  localparam realtime T [2] = '{1.5ns, 2us};
  localparam time D [2] = '{3ns, 4ns};
  parameter bit [7:0] B [2] = '{8'h1, 8'h2};
  int i = 1;
  initial begin
    $display("%0.3f %0.3f %0.3f", T[0], T[1], T[i]);
    $display("%0d %0d", D[0], D[i]);
    $display("%0d", B[i+3]);
    #(D[i]) $display("%0t", $realtime);
  end
endmodule
