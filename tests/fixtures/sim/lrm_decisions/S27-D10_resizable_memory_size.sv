// Decision S27-D10: a dynamic array or queue loads and dumps at its current
// size, with addresses 0 to size-1; a load never resizes it. A file longer
// than the array leaves the extra words unread, and an empty array loads
// nothing.
//
// IEEE 1800-2009 21.4.1 (SystemVerilog-1800-2009.txt L37347):
//   "When loading dynamic arrays and queues, the current size of the array is
//   fixed; the array shall not be resized."
//
// The text does not say what loading an empty array does; llg loads nothing
// and warns.
module tb;
  logic [7:0] dyn [];
  logic [7:0] fifo [$];
  logic [7:0] empty [];
  logic [7:0] back [0:2];
  integer fd;
  initial begin
    fd = $fopen("s27_d10.mem", "w");
    $fwrite(fd, "11\n22\n33\n44\n");
    $fclose(fd);
    dyn = new[3];
    fifo = '{8'h01, 8'h02};
    $readmemh("s27_d10.mem", dyn);
    $readmemh("s27_d10.mem", fifo, 1, 0);
    $readmemh("s27_d10.mem", empty);
    $display("dyn %h %h %h size %0d", dyn[0], dyn[1], dyn[2], dyn.size());
    $display("queue %h %h size %0d", fifo[0], fifo[1], fifo.size());
    $display("empty size %0d", empty.size());
    $writememh("s27_d10_dump.mem", dyn);
    $readmemh("s27_d10_dump.mem", back);
    $display("dump %h %h %h", back[0], back[1], back[2]);
    $finish;
  end
endmodule
