// SV2009 7.4.6, 11.5, 23.2.2.3: member selections below a fixed row.
typedef struct packed { logic [3:0] pad; logic [0:7] data; } record_t;
module child(ref logic [0:3] x);
 initial begin #1; x[1 +: 2] = 2'b10; x[4] = 1; end
endmodule
module tb;
 record_t rows [2:1];
 child c(rows[1].data[2 +: 4]);
 initial begin
  rows[2] = 12'habc;
  rows[1] = 12'hf00;
  #2;
  $display("rows %h %h", rows[2], rows[1]);
  $finish(0);
 end
endmodule
