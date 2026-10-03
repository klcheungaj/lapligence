// SV2009 10.4.1/10.4.2, 11.4.2, 13.5: capture destinations once before writes/calls.
module child(ref logic [7:0] x);
 int count = 0;
 int slot = 0;
 logic [1:0] result;
 function automatic int index_once(); count++; return slot; endfunction
 task automatic copy_out(output logic [1:0] result);
  slot = 4;
  #1;
  result = 2'b10;
 endtask
 initial begin
  #1;
  x[index_once() +: 2] = 2'b01;
  $display("blocking %0d %h", count, x);
  x[index_once() +: 2] += 1;
  $display("mutating %0d %h", count, x);
  x[index_once() +: 2] <= 2'b11;
  slot = 6;
  #1;
  $display("nba %0d %h", count, x);
  slot = 2;
  copy_out(x[index_once() +: 2]);
  $display("copyout %0d %h", count, x);
  result = x[index_once() +: 2]++;
  $display("postfix %0d %0d %h", count, result, x);
  result = ++x[index_once() +: 2];
  $display("prefix %0d %0d %h", count, result, x);
  x[2 +: 4] <= x[0 +: 4];
  x[0 +: 4] = 0;
  #1;
  $display("snapshot %0d %h", count, x);
 end
endmodule
module tb;
 logic [15:0] value = 0;
 child c(value[11:4]);
 initial begin #5; $display("root %h", value); $finish(0); end
endmodule
