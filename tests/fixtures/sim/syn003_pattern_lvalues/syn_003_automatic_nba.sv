// llg-test-fixture: SYN-003 automatic NBA target rejection.
// LRM: IEEE 1800-2009 10.9.
module tb;
  typedef logic [7:0] U [0:1];
  U values;
  task automatic assign_automatic(input U source);
    logic [7:0] local_value;
    '{local_value, local_value} <= source;
  endtask
  initial begin
    assign_automatic(values);
    $finish;
  end
endmodule
