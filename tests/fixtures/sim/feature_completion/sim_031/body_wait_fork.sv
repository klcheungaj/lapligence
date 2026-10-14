`timescale 1ns / 1ns
// The chosen body is an ordinary statement: it can wait on a delay or an
// event, fork children, declare locals that live across those waits and call
// timed tasks. Unchosen bodies (zero weights) produce no effects at all.
module tb;
  event ev;
  int unchosen;

  task automatic slow(input int id);
    #2 $display("t=%0t slow %0d", $time, id);
  endtask

  task automatic through_task(input int w);
    randcase
      w : begin
            int local_value = 40 + w;
            #3 $display("t=%0t task body local=%0d", $time, local_value);
            slow(w);
            $display("t=%0t task body done local=%0d", $time, local_value);
          end
      0 : begin unchosen++; #1; end
    endcase
  endtask

  initial begin
    randcase
      0 : begin unchosen++; $display("unchosen A"); #1; end
      1 : begin #5 $display("t=%0t chosen after delay", $time); end
      0 : $display("unchosen C");
    endcase
    $display("t=%0t continued", $time);

    randcase
      1 : begin
            fork
              begin #2 $display("t=%0t child 1", $time); end
              begin #4 $display("t=%0t child 2", $time); end
            join
            $display("t=%0t joined", $time);
          end
      0 : fork unchosen++; join
    endcase

    fork
      begin #3 -> ev; end
    join_none
    randcase
      1 : begin
            @ev;
            $display("t=%0t event seen", $time);
            slow(1);
          end
      0 : @ev;
    endcase

    through_task(1);

    // Two processes run the same task with different roles concurrently.
    fork
      through_task(2);
      begin #1 $display("t=%0t other process", $time); end
    join
    $display("unchosen=%0d", unchosen);
    $finish;
  end
endmodule
