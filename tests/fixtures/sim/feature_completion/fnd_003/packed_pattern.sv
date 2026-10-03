// IEEE 1800-2009 sections 7.2.1, 10.9.2: packed structure assignment patterns.
module tb;
  typedef struct packed { logic [7:0] payload; bit [1:0] flags; } packet_t;
  packet_t original, copied;
  initial begin
    original = '{payload: 8'hax, flags: 2'bxz};
    copied = original;
    original.payload = 8'h34;
    $display("copied=%h flags=%b original=%h", copied.payload, copied.flags, original.payload);
    $finish(0);
  end
endmodule
