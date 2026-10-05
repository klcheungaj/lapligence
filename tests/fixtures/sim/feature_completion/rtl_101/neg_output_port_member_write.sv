// RTL-101 nearest illegal: an output port is an implied continuous
// assignment (SV 23.3.3.2), so the connected record has no other writer.
typedef struct { logic [1023:0] w [0:2047]; bit [7:0] t; } big_t;
module src(output big_t o);
  initial o.t = 8'd1;
endmodule
module tb;
  big_t q;
  src s(.o(q));
  initial begin
    q.w[4] = 1024'd2;
    $finish(0);
  end
endmodule
