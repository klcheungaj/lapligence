// Decision CD-D1: `disable` of a named block inside a class method, or of a
// class task, ends every active execution of that block or task: in every
// object and in every process, not only the object or process that executes
// the `disable`. Execution resumes after the block (or after the task call);
// a disable of a block that is not executing has no effect.
//
// IEEE 1800-2009 9.6.2 (SystemVerilog-1800-2009.txt L12387-12391): "The
// disable statement shall terminate the activity of a task or a named block.
// Execution shall resume at the statement following the block or following
// the task-enabling statement. ... If a task is enabled more than once, then
// disabling such a task shall disable all activations of the task." L12402-
// 12403: "Disabling an automatic task or a block inside an automatic task
// proceeds as for regular tasks for all concurrent executions of the task."
// L12441: "If the block is not currently executing, the disable has no
// effect." 8.6 L9766: "The lifetime of methods declared as part of a class
// type shall be automatic." The text does not mention objects: llg reads
// "all concurrent executions" as the executions of the method in every
// object, so `a.stop()` also ends `b`'s block.
`timescale 1ns/1ns

class Worker;
  static int entered;
  static int finished;
  static int resumed;
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
endclass

class Spinner;
  static int laps;
  task spin();
    forever begin
      #2;
      laps++;
    end
  endtask
  task halt();
    disable spin;
  endtask
endclass

module tb;
  initial begin
    static Worker a = new;
    static Worker b = new;
    static Spinner s1 = new;
    static Spinner s2 = new;
    a.stop();
    fork
      a.run(10);
      b.run(20);
      #5 a.stop();
    join
    $display("%0d entered=%0d finished=%0d resumed=%0d", $time, Worker::entered,
             Worker::finished, Worker::resumed);
    fork
      s1.spin();
      s2.spin();
      #7 s1.halt();
    join
    $display("%0d laps=%0d", $time, Spinner::laps);
    $finish;
  end
endmodule
