// SIM-025 A03: nearest illegal form of static_subroutines.sv. Variables of an
// automatic task die with the activation, so they "shall not be traced with
// system tasks such as $monitor" (SV 13.3.2).
module tb;
  task automatic t(input [3:0] v);
    $monitor("v=%0d", v);
  endtask
  initial begin
    t(4'd1);
    #1 $finish(0);
  end
endmodule
