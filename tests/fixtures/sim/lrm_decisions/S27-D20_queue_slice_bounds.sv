// Decision S27-D20: the bounds of a queue slice used as memory_name may be
// any integral expressions, and `$` in them is the last index of the queue
// when the task runs. Each bound is evaluated once, left first. The slice
// addresses those elements in place and never resizes the queue. A slice
// whose left bound exceeds its right bound selects no element; llg rejects
// that call, like a slice outside the current size (S27-D14): nothing is
// loaded, and a dump writes no file.
//
// IEEE 1800-2009 7.10 (SystemVerilog-1800-2009.txt L8977-8978):
//   "Each element in a queue is identified by an ordinal number that
//   represents its position within the queue, with 0 representing the first,
//   and $ representing the last."
// IEEE 1800-2009 7.10.1 (L9028-9029, L9034-9035):
//   "In a queue slice expression such as Q[a:b], the slice bounds may be
//   arbitrary integral expressions and, in particular, are not required to be
//   constant expressions."
//   "Q[ a : b ] yields a queue with b - a + 1 elements.
//    If a > b, then Q[a:b] yields the empty queue {}."
// IEEE 1800-2009 21.4.1 (L37347):
//   "When loading dynamic arrays and queues, the current size of the array is
//   fixed; the array shall not be resized."
//
// The text does not say what a memory task does with an empty slice; llg
// rejects it, as it rejects a slice outside the current size (S27-D14).
module tb;
  logic [7:0] q [$];
  logic [7:0] back [0:1];
  integer fd;
  integer i;

  initial begin
    fd = $fopen("s27_d20.mem", "w");
    $fwrite(fd, "11\n22\n");
    $fclose(fd);
    q = '{8'h01, 8'h02, 8'h03, 8'h04, 8'h05};
    i = 3;
    $readmemh("s27_d20.mem", q[i:$]);
    $display("q %h %h %h %h %h size %0d", q[0], q[1], q[2], q[3], q[4], q.size());
    $readmemh("s27_d20.mem", q[$-4:i-2]);
    $display("q %h %h %h %h %h size %0d", q[0], q[1], q[2], q[3], q[4], q.size());
    $writememh("s27_d20_dump.mem", q[$-1:$]);
    // Reversed bounds select no element: nothing happens.
    $readmemh("s27_d20.mem", q[i:1]);
    $writememh("s27_d20_dump.mem", q[$:i-1]);
    $display("q %h %h %h %h %h size %0d", q[0], q[1], q[2], q[3], q[4], q.size());
    $readmemh("s27_d20_dump.mem", back);
    $display("dump %h %h", back[0], back[1]);
    $finish;
  end
endmodule
