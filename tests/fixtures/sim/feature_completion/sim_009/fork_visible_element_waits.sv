// SIM-009/SIM-027: event controls and waits in fork branches on elements of
// a fork-visible automatic array (IEEE 1800-2009 9.3.2, 9.4.2, 9.4.3). Each
// waiter is woken at its own time, so the output order is fixed.
module tb;
  task automatic run();
    logic [7:0] mem [0:3];
    int i;
    i = 1;
    mem = '{default: 8'h00};
    fork
      begin @(mem[0]); $display("%0t @(mem[0]) %h", $time, mem[0]); end
      begin @(mem[i]); $display("%0t @(mem[i]) %h", $time, mem[i]); end
      begin wait (mem[1] == 8'h22); $display("%0t wait %h", $time, mem[1]); end
      begin @(posedge mem[3][0]); $display("%0t posedge %h", $time, mem[3]); end
      begin @(mem[0] iff mem[1] == 8'h44); $display("%0t iff %h", $time, mem[0]); end
      begin
        fork
          begin @(mem[2]); $display("%0t nested %h", $time, mem[2]); end
        join
      end
      begin
        #1 mem[0] = 8'h11;
        #1 mem[1] = 8'h05;
        #1 mem[1] = 8'h22;
        #1 mem[3] = 8'h01;
        #1 mem[2] = 8'h33;
        #1 mem[1] = 8'h44;
        #1 mem[0] = 8'h12;
      end
    join
    fork
      begin @(mem[2]); $display("%0t detached %h", $time, mem[2]); end
    join_none
    #1 $readmemh("fork_waits.mem", mem, 2, 2);
    wait fork;
    $display("%0t done %h %h %h %h", $time, mem[0], mem[1], mem[2], mem[3]);
  endtask

  // A variable that a later detached fork shares is still a borrowed
  // capture of a joined branch, which can wait on it.
  task automatic shared_later();
    logic [7:0] x;
    logic [7:0] row [0:1];
    x = 8'h00;
    row[1] = 8'h00;
    fork
      begin @(x); $display("%0t joined x %h", $time, x); end
      begin @(row[1]); $display("%0t joined row[1] %h", $time, row[1]); end
      begin #1 x = 8'h01; #1 row[1] = 8'h02; end
    join
    fork
      begin @(x or row[1]); $display("%0t detached %h %h", $time, x, row[1]); end
    join_none
    #1 x = 8'h03;
    wait fork;
  endtask

  initial begin
    int fd;
    fd = $fopen("fork_waits.mem", "w");
    $fwrite(fd, "aa\n");
    $fclose(fd);
    run();
    shared_later();
    $finish;
  end
endmodule
