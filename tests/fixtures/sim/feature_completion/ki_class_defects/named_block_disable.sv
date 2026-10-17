// class-defects item 1: `disable` of a named block of a class method from
// another process. Expected lines are derived in readme.md.
`timescale 1ns/1ps
class Worker;
  static int entered;
  static int finished;
  static int resumed;
  int id;
  function new(int i);
    id = i;
  endfunction
  task run(int delay);
    begin : wait_blk
      entered++;
      #(delay);
      finished++;
    end
    resumed++;
  endtask
  task stop();
    disable run.wait_blk;
  endtask
  // A disable of a block nobody is executing has no effect (9.6.2).
  task stop_idle();
    disable idle.blk;
  endtask
  task idle();
    begin : blk
      #1;
    end
  endtask
endclass

class Gen #(int N = 1);
  static int laps;
  task spin();
    forever begin : lap
      #(N);
      laps++;
    end
  endtask
  task halt();
    disable spin;
  endtask
endclass

class Ticker;
  int n;
  virtual task spin();
    forever begin
      #2;
      n++;
    end
  endtask
endclass

class FastTicker extends Ticker;
  virtual task spin();
    forever begin
      #2;
      n += 10;
    end
  endtask
  task halt();
    disable spin;
  endtask
endclass

module tb;
  initial begin
    static Worker a = new(1);
    static Worker b = new(2);
    static Worker c = new(3);
    static Gen #(2) g = new;
    static FastTicker f = new;
    static Ticker t = f;
    fork
      a.run(10);
      b.run(20);
      begin
        #3 c.stop_idle();
        #2 c.stop();
      end
    join
    $display("%0t entered=%0d finished=%0d resumed=%0d", $time, Worker::entered,
             Worker::finished, Worker::resumed);
    c.run(4);
    $display("%0t entered=%0d finished=%0d resumed=%0d", $time, Worker::entered,
             Worker::finished, Worker::resumed);
    fork
      g.spin();
      #7 g.halt();
    join
    $display("%0t laps=%0d", $time, Gen#(2)::laps);
    fork
      t.spin();
      #5 f.halt();
    join
    $display("%0t n=%0d", $time, f.n);
    $finish;
  end
endmodule
