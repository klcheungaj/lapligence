// llg-test-fixture: tests/fixtures/sim/process_control/bound_updates_reclaimed.sv
// Decision S09-D8: timed triggers and the updates of class handles and static
// class properties end with their issuer. Each iteration ends the issuers of
// one set in another way: a kill of the live or of the finished issuer, a
// disable fork, a disable of the block they have left, a commit. The updates
// between them that stay commit in issue order, nothing dropped wakes its
// event or is waited for, and the records kept for finished issuers are
// reclaimed: the run ends with the six static processes (the `$finish(2)`
// statistics line), with updates of every kind still pending.
`timescale 1ns/1ns
class item;
  static int count;
endclass

module tb;
  event now, later, changed, twice;
  int n_now, n_later, n_changed, n_twice, n_handle, seq, order;
  item h, made = new;
  logic go = 0;
  process issuer;

  always @(now) n_now++;
  always @(later) n_later++;
  always @(changed) n_changed++;
  always @(twice) n_twice++;
  always @(h) n_handle++;

  task automatic issue(int i);
    seq <= seq * 10 + 1;
    ->> #0 now;
    item::count <= i;
    order <= 1;
    h <= i % 2 ? made : null;
    order <= 2;
    ->> #3 later;
    ->> @(posedge go) changed;
    ->> repeat (2) @(go) twice;
    order <= 3;
  endtask

  initial begin
    for (int i = 0; i < 2000; i++) begin : iteration
      automatic int mode = i % 5;
      order = 0;
      begin : spawned
        fork
          begin
            issuer = process::self();
            issue(i);
            if (mode == 0) #100;
          end
        join_none
        #0;
      end
      case (mode)
        0, 1: issuer.kill();
        2: disable fork;
        3: disable spawned;
        default: ;
      endcase
      #1 if (order != 3) $display("iteration %0d: order %0d", i, order);
      go = 1;
      #1 go = 0;
      #3;
    end
    $display("now=%0d later=%0d changed=%0d twice=%0d handle=%0d count=%0d",
             n_now, n_later, n_changed, n_twice, n_handle, item::count);
    // A trigger that is dropped leaves no time to advance to.
    fork
      begin
        issuer = process::self();
        ->> #500 later;
      end
    join_none
    #1 issuer.kill();
    issue(7);
    ->> #400 later;
    ->> @(negedge go) changed;
    #1 $display("pending at the end");
    $finish(2);
  end
endmodule
