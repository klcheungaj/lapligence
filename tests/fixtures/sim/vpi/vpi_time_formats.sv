// Static-review regression, not executed during patch preparation.
`timescale 10ns/1ns
module coarse;
  logic value;
endmodule
`timescale 10ps/1ps
module fine;
  logic value;
endmodule
`timescale 1ns/1ps
module tb;
  coarse c();
  fine f();
  initial begin
    #2;
    $vpi_time_formats();
    $finish(0);
  end
endmodule
