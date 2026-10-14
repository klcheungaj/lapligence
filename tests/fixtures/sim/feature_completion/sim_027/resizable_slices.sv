// SIM-027 A03: slices of dynamic arrays and queues as memory_name (IEEE
// 1800-2009 21.4, 21.4.1, 21.5). A slice addresses the selected elements in
// place with the container's ascending indices; the array is never resized,
// and a slice outside the current size is rejected at run time (decision
// S27-D14).
module tb;
  class holder;
    logic [7:0] cells [];
  endclass
  logic [7:0] fifo [$];
  logic [7:0] dyn [];
  logic [7:0] back [0:3];

  task automatic local_queue;
    logic [7:0] q [$];
    q = '{8'ha0, 8'ha1, 8'ha2};
    $readmemh("words.mem", q[1:2], 2, 1);
    $display("local queue %h %h %h size %0d", q[0], q[1], q[2], q.size());
    $writememh("dump.mem", q[0:1]);
  endtask

  initial begin
    int fd;
    holder h;
    fd = $fopen("words.mem", "w");
    $fwrite(fd, "11\n22\n33\n44\n");
    $fclose(fd);
    fifo = '{8'h01, 8'h02, 8'h03, 8'h04, 8'h05};
    dyn = new[6];
    $readmemh("words.mem", fifo[1:2]);
    $display("queue %h %h %h %h %h size %0d", fifo[0], fifo[1], fifo[2], fifo[3], fifo[4],
             fifo.size());
    $readmemh("words.mem", dyn[1:4]);
    $display("dyn %h %h %h %h %h %h", dyn[0], dyn[1], dyn[2], dyn[3], dyn[4], dyn[5]);
    $readmemh("words.mem", dyn[2+:3], 4, 2);
    $display("dyn %h %h %h %h %h %h", dyn[0], dyn[1], dyn[2], dyn[3], dyn[4], dyn[5]);
    $writememh("dump.mem", fifo[3:4]);
    $readmemh("dump.mem", back);
    $display("queue dump %h %h %h %h", back[0], back[1], back[2], back[3]);
    $writememh("dump.mem", dyn[4-:2], 3, 4);
    $readmemh("dump.mem", back);
    $display("dyn dump %h %h %h %h", back[0], back[1], back[2], back[3]);
    local_queue();
    $readmemh("dump.mem", back);
    $display("local dump %h %h %h %h", back[0], back[1], back[2], back[3]);
    h = new;
    h.cells = new[3];
    $readmemh("words.mem", h.cells[1:2]);
    $display("property %h %h %h", h.cells[0], h.cells[1], h.cells[2]);
    // Outside the current size: rejected, nothing loaded or dumped.
    $readmemh("words.mem", fifo[4:6]);
    $writememh("dump.mem", dyn[5:6]);
    $readmemh("dump.mem", back);
    $display("rejected %h size %0d dump %h %h", fifo[4], fifo.size(), back[0], back[1]);
    $finish;
  end
endmodule
