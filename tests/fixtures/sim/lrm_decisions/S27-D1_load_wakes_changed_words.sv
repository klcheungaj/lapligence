// Decision S27-D1: a memory load stores each word when the task reaches it.
// A word whose value changes is an update event, so every process waiting
// on that word wakes once in the load's time slot and reads the loaded
// value; a word loaded with the value it already holds is no event, and
// words outside the range are untouched. The loading process itself sees
// every loaded word in its next statement.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37280):
//   "As the file is read, each number encountered is assigned to a successive
//   word element of the memory."
// IEEE 1800-2009 4.3 (L3039-3043):
//   "Every change in state of a net or variable in the system description
//   being simulated is considered an update event."
//   "Processes are sensitive to update events. When an update event is
//   executed, all the processes that are sensitive to that event are
//   considered for evaluation in an arbitrary order."
//
// The order in which the woken processes run is not fixed (4.7), so they
// only count their wakes; the counts are printed one time unit later.
module tb;
  logic [7:0] mem [0:3];
  int wakes [0:3];
  integer fd;
  always @(mem[0]) wakes[0]++;
  always @(mem[1]) wakes[1]++;
  always @(mem[2]) wakes[2]++;
  always @(mem[3]) wakes[3]++;
  initial begin
    fd = $fopen("s27_d1.mem", "w");
    $fwrite(fd, "11\n22\n33\n");
    $fclose(fd);
    mem = '{8'h00, 8'h22, 8'h00, 8'h00};
    #1;
    wakes = '{0, 0, 0, 0};
    $readmemh("s27_d1.mem", mem, 0, 2);
    $display("caller sees %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    #1;
    $display("wakes %0d %0d %0d %0d", wakes[0], wakes[1], wakes[2], wakes[3]);
    $finish;
  end
endmodule
