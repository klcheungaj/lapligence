module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic [7:0] data = 0;
  logic positive = 0;
  logic negative = 0;
  event marker;

  default clocking cb @(posedge clk);
    input #0 data;
    output positive;
  endclocking
  clocking prior @(posedge clk);
    input #1step data;
  endclocking
  clocking cb_neg @(negedge clk);
    output negative;
  endclocking

  initial begin
    #1;
    data <= 9;
    clk = 1;
    -> marker;
    #0 clk = 0;
    #0 clk = 1;
    #0;
    if (!marker.triggered) $fatal(1, "same-slot triggered");
    ##0 cb.positive <= 1;
    cb_neg.negative <= 1;
    #1;
    if (marker.triggered) $fatal(1, "triggered survived time advance");
    $display("settled t=%0t pos=%0d neg=%0d", $time, positive, negative);
    ##0 cb.positive <= 0;
    #0;
    $display("next t=%0t", $time);
    #1;
    $display("final t=%0t pos=%0d", $time, positive);
    $finish(0);
  end

  initial begin
    @(cb);
    $display("sample t=%0t zero=%0d prior=%0d", $time, cb.data, prior.data);
    @(cb);
    $display("sample t=%0t zero=%0d prior=%0d", $time, cb.data, prior.data);
  end

  initial begin
    #3 clk = 0;
    data <= 7;
    #0 clk = 1;
  end
endmodule
