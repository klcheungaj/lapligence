// Decision S27-D14: a slice of a dynamic array or queue is a valid
// memory_name. It addresses the selected elements in place with the array's
// own indices and never resizes the array. A slice that is not within the
// current size is an error: nothing is loaded, and a dump writes no file.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37253-37254):
//   "The lowest dimension (i.e., the rightmost specified dimension in the
//   identifier) can be specified with slice syntax."
// IEEE 1800-2009 21.4.1 (L37347):
//   "When loading dynamic arrays and queues, the current size of the array is
//   fixed; the array shall not be resized."
// IEEE 1800-2009 7.4.6 (L8235-8237):
//   "If an index expression is out of the address bounds or if any bit in the
//   address is X or Z, then the index shall be invalid. ... Writing to an
//   array with an invalid index shall perform no operation."
//
// The text does not say what a memory task does with a slice that is partly
// or wholly outside the current size; llg rejects the call, as it rejects a
// start or finish address outside a fixed array (S27-D7, S27-D12).
module tb;
  logic [7:0] fifo [$];
  logic [7:0] dyn [];
  logic [7:0] back [0:1];
  integer fd;
  initial begin
    fd = $fopen("s27_d14.mem", "w");
    $fwrite(fd, "11\n22\n");
    $fclose(fd);
    fifo = '{8'h01, 8'h02, 8'h03, 8'h04};
    dyn = new[4];
    $readmemh("s27_d14.mem", fifo[1:2]);
    $display("queue %h %h %h %h size %0d", fifo[0], fifo[1], fifo[2], fifo[3], fifo.size());
    $readmemh("s27_d14.mem", dyn[2:3], 3, 2);
    $display("dyn %h %h size %0d", dyn[2], dyn[3], dyn.size());
    $readmemh("s27_d14.mem", fifo[3:4]);
    $display("outside queue %h size %0d", fifo[3], fifo.size());
    $writememh("s27_d14_dump.mem", fifo[0:1]);
    $writememh("s27_d14_dump.mem", dyn[3:4]);
    $readmemh("s27_d14_dump.mem", back);
    $display("dump %h %h", back[0], back[1]);
    $finish;
  end
endmodule
