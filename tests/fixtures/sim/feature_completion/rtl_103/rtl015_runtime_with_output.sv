// IEEE 1800-2009 11.4.14.4, 13.5: formerly the RTL-015 negative
// `neg_runtime_with_output`. A runtime `with` range on a copy-out actual
// receives the formal's value in the selected elements.
module tb;
  logic [7:0] q [0:3];
  int i;
  task automatic produce(output logic [15:0] value);
    value = 16'h1234;
  endtask
  initial begin
    q = '{default: 8'h00};
    i = 1;
    produce({>>{q with [i +: 2]}});
    $display("%h %h %h %h", q[0], q[1], q[2], q[3]);
    $finish(0);
  end
endmodule
