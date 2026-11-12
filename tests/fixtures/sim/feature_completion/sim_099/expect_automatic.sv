// IEEE 1800-2009 16.18: an expect statement can refer to automatic
// variables, here the formal of an automatic task (the wait_for example of
// the clause). Its value is the one the variable holds when the statement
// starts its attempt. readme.md derives every line.
`timescale 1ns/1ns
module tb;
  bit clk;
  always #5 clk = ~clk;
  int data = 0;
  task automatic wait_for(int value, output bit ok);
    expect (@(posedge clk) ##[1:4] data == value) ok = 1;
    else ok = 0;
  endtask
  initial begin
    bit ok;
    wait_for(3, ok);
    $display("[%0t] ok=%0d", $time, ok);
    wait_for(9, ok);
    $display("[%0t] ok=%0d", $time, ok);
    $finish;
  end
  always @(posedge clk) data <= data + 1;
endmodule
