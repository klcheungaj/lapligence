// llg-test-fixture: SYN-003 automatic ref NBA target rejection.
// LRM: IEEE 1800-2009 10.9.
module tb;
  typedef logic [7:0] U [0:1];
  U values;
  task automatic assign_ref(ref logic [7:0] target, input U source);
    '{target, target} <= source;
  endtask
  initial begin
    logic [7:0] value;
    assign_ref(value, values);
    $finish;
  end
endmodule
