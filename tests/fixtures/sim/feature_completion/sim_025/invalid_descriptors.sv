// SIM-025 A03: a monitor or strobe on an invalid, unknown or closed
// descriptor is ignored; the failure is reported through $ferror and the
// simulation continues with its other monitors (SV 21.3.1, 21.3.7).
module tb;
  integer f, err;
  string msg;
  reg [31:0] unknown = 32'hx;
  reg [3:0] a = 1;
  initial begin
    $monitor("ok a=%0d", a);
    $fmonitor(0, "zero %0d", a);
    $fmonitor(unknown, "unknown %0d", a);
    $fmonitor(32'h4000_0000, "unopened channel %0d", a);
    $fmonitor(32'h8000_007f, "unopened fd %0d", a);
    $fstrobe(0, "zero strobe %0d", a);
    $fstrobe(32'h8000_007f, "unopened fd strobe %0d", a);
    f = $fopen("sim025_c.txt");
    $fclose(f);
    $fmonitor(f, "closed %0d", a);
    $fstrobe(f, "closed strobe %0d", a);
    err = $ferror(f, msg);
    $display("closed err=%0d", err != 0);
    #1 a = 2;
    #1 $finish(0);
  end
endmodule
