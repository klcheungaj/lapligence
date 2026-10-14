`timescale 1ns / 1ns
// return, disable, break and continue inside a chosen body leave the
// enclosing construct exactly as in any other statement; a process killed
// while its chosen body waits never runs the rest of the body, and unchosen
// bodies never run.
module tb;
  int unchosen;
  int loop_count;
  int survivor_a;
  int survivor_b;
  event never;

  function automatic int early(input int w, input int fallback);
    randcase
      w : return 100 + w;
      0 : unchosen++;
    endcase
    return fallback;
  endfunction

  task automatic task_return(input int w);
    randcase
      w : begin
            #2;
            $display("t=%0t task_return before", $time);
            return;
          end
    endcase
    $display("t=%0t task_return fell through", $time);
  endtask

  initial begin : main
    $display("early(1)=%0d early(0)=%0d", early(1, -1), early(0, -1));
    task_return(1);
    task_return(0);

    begin : escape
      randcase
        1 : begin
              $display("in escape");
              disable escape;
              $display("unreachable after disable");
            end
        0 : unchosen++;
      endcase
      $display("unreachable after randcase");
    end
    $display("after escape");

    for (int i = 0; i < 6; i++) begin
      randcase
        1 : begin
              if (i == 1) continue;
              if (i == 4) break;
              loop_count++;
            end
      endcase
    end
    $display("loop_count=%0d", loop_count);

    // Another process disables the block while its chosen body waits.
    fork
      begin : victim
        randcase
          1 : begin
                int held = 5;
                #10 survivor_a = held;
                $display("victim body finished");
              end
          0 : unchosen++;
        endcase
        survivor_b = 1;
      end
      begin
        #3 disable victim;
        $display("t=%0t victim disabled", $time);
      end
    join
    #20;
    $display("survivors=%0d,%0d unchosen=%0d", survivor_a, survivor_b, unchosen);

    // disable fork cancels a child that waits inside a chosen body.
    fork
      randcase
        1 : begin @never; $display("child should not wake"); end
      endcase
    join_none
    #1 disable fork;
    -> never;
    #1 $display("t=%0t end", $time);
    $finish;
  end
endmodule
