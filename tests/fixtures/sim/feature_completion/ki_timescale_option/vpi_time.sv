// A VPI application reports the time unit and precision of `tb` and of the
// simulation (a NULL object) and the time after one unit.
module tb;
  initial begin
    #1 $ts_report;
    $finish;
  end
endmodule
