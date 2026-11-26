// Event queues of event queues declared in a named procedural block and
// in a static task.
`timescale 1ns/1ns
module tb;
  event e1, e2;
  task static st();
    event sw[$][$];
    sw.push_back({e2});
    @(sw[0][0]);
    $display("st at %0t", $time);
  endtask
  initial begin : blk
    event bw[$][$];
    event bf[2][$];
    bw.push_back({e1});
    bf[1] = {e2, e1};
    fork
      begin @(bw[0][0]); $display("bw at %0t", $time); end
      begin @(bf[1][1]); $display("bf at %0t", $time); end
      st();
    join_none
    #1 -> e1;
    #1 -> e2;
    #1 $display("%0d %0d", bw[0].size(), bf[1].size());
  end
endmodule
