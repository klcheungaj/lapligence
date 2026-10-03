// IEEE 1800-2009 11.4.14.4, 13.5: a runtime `with` range on a copy-out actual
// needs the checked streaming-assignment path, which output copy-out does not
// use; it is rejected instead of ignoring the range.
module tb;
  logic [7:0] q [0:3];
  int i;
  task automatic produce(output logic [15:0] value);
    value = 16'h1234;
  endtask
  initial begin
    i = 1;
    produce({>>{q with [i +: 2]}});
    $finish;
  end
endmodule
