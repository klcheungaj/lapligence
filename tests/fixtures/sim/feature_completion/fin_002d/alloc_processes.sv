// Allocation failure matrix: processes, coroutine frames of nested and
// recursive timed calls, fork forms, process handles and cancellation.
`timescale 1ns / 1ns
module tb;
  int log[$];
  int depth_reached;
  logic clk = 1'b0;
  logic [7:0] counter = '0;
  process handles[$];
  int ids[$];

  always #5 clk = ~clk;
  always @(posedge clk) counter <= counter + 8'd1;

  task automatic descend(input int depth, input int delay, ref int reached);
    int local_values[4];
    string label;
    if (depth > reached) reached = depth;
    label = $sformatf("d%0d", depth);
    local_values[depth % 4] = label.len();
    #delay;
    if (depth < 30) descend(depth + 1, delay, reached);
    log.push_back(depth + local_values[depth % 4]);
  endtask

  task automatic worker(input int id);
    process self = process::self();
    handles.push_back(self);
    ids.push_back(id);
    repeat (id + 1) @(posedge clk);
    log.push_back(100 + id);
  endtask

  initial begin
    descend(0, 1, depth_reached);
    $display("depth %0d log %0d at %0t", depth_reached, log.size(), $time);
    for (int i = 0; i < 8; i++) begin
      fork
        automatic int id = i;
        worker(id);
      join_none
    end
    wait fork;
    $display("workers %0d handles %0d at %0t", log.size(), handles.size(), $time);
    fork
      begin : slow
        #100 log.push_back(1);
      end
      begin : fast
        #3 log.push_back(2);
      end
      begin
        fork
          #1 log.push_back(3);
          #2 log.push_back(4);
        join
      end
    join_any
    disable fork;
    $display("any %0d last %0d", log.size(), log[$]);
    fork
      begin : victim
        forever @(posedge clk) log.push_back(5);
      end
      begin
        repeat (3) @(negedge clk);
        disable victim;
      end
    join
    fork
      worker(20);
      worker(21);
    join_none
    #1;
    begin
      // The two workers start in an order the standard leaves open (4.7):
      // the one of id 21 is killed and the one of id 20 awaited.
      automatic int last = ids.size() - 1;
      automatic process victim = ids[last] == 21 ? handles[last] : handles[last-1];
      automatic process survivor = ids[last] == 21 ? handles[last-1] : handles[last];
      victim.kill();
      survivor.await();
      $display("kill %0d %0d %0d counter %0d", log.size(), victim.status() == process::KILLED,
               survivor.status() == process::FINISHED, counter);
    end
    $display("done");
    $finish;
  end
endmodule
