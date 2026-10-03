// SV2009 11.5, 13.5.2, 21.3.4.3: scans through legal ref variables and selected views.
module child(ref logic [7:0] x);
 int count = 0;
 int selected = 2;
 function automatic int once(); count++; return selected; endfunction
 task automatic scan(ref logic [7:0] word);
  int n;
  logic [31:0] unknown_index;
  n = $sscanf("a", "%h", word[once() +: 4]);
  $display("scan %0d %h %0d", n, word, count);
  selected = -1;
  n = $sscanf("7", "%d", word[once() +: 3]);
  unknown_index = 'x;
  n = $sscanf("0", "%d", word[unknown_index +: 2]);
  $display("partial %0d %h %0d", n, word, count);
  n = $sscanf("bad", "%d", word[once() +: 3]);
  $display("failed %0d %h %0d", n, word, count);
 endtask
 initial begin #1; scan(x); end
endmodule
module tb;
 logic [15:0] value = 16'h0810;
 child c(value[11:4]);
 initial begin #2; $display("root %h", value); $finish(0); end
endmodule
