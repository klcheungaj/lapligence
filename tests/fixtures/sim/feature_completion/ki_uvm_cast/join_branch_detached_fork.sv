`timescale 1ns/1ns
// 9.3.2: a detached fork inside a joined branch shares the automatics of the
// declaring task, also after the joined branch and the task have ended.
class holder;
  int base = 5;
  task run(int n);
    int total = n;
    real half = 0.5;
    string text = "s";
    fork
      begin
        fork
          begin
            #1 total = total + base;
            half = half + 1;
            text = {text, "i"};
            $display("%0t inner %0d %0.1f %s", $time, total, half, text);
            #4 total = total + 1;
            $display("%0t late %0d %0.1f %s", $time, total, half, text);
          end
        join_none
        #2 total = total + 10;
        $display("%0t mid %0d %0.1f %s", $time, total, half, text);
      end
      begin
        fork
          fork
            #4 $display("%0t deep %0d", $time, total);
          join_none
        join
        #3 $display("%0t other %0d", $time, total);
      end
    join
    $display("%0t joined %0d %0.1f %s", $time, total, half, text);
  endtask
endclass
module tb;
  initial begin
    automatic holder h = new;
    h.run(3);
    $display("%0t returned", $time);
    #5 $display("%0t end", $time);
  end
endmodule
