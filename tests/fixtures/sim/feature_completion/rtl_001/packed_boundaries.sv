// SV2009 sections 7.2.1, 7.4.1: member domains and signed/reversed packed bounds.
module tb;
  typedef struct packed { bit [2:-1] flags; logic signed [66:2] payload; } packet_t;
  packet_t original, copied;
  integer index;
  initial begin
    original.flags = 4'bxz10;
    original.payload = 65'h1ffffffffffffffff;
    copied = original;
    index = 2;
    copied.payload[index +: 64] = 64'h123456789abcdef0;
    index = -1;
    copied.flags[index] = 1'b1;
    index = 99;
    copied.payload[index] = 1'b0;
    $display("original=%b:%h signed=%0d", original.flags, original.payload, original.payload);
    $display("copied=%b:%h", copied.flags, copied.payload);
    copied.payload[66:63] = 4'h0;
    $display("boundary=%h original=%h", copied.payload, original.payload);
    $display("invalid=%b kept=%b", copied.payload[-7], copied.flags);
    $finish(0);
  end
endmodule
