// randsequence is a statement (IEEE 1800-2009 18.17, statement_item), so it
// runs in functions, tasks, class methods, always blocks and concurrent
// processes; code blocks of a task's randsequence may wait. Weighted choices
// are checked by range, the rest exactly.
module tb;
  timeunit 1ns;
  timeprecision 1ns;
  logic clk = 0;
  int edges;

  class Gen;
    int count;
    string log;
    function int run(int n);
      int total = 0;
      randsequence (main)
        main : repeat (n) item ;
        item : little := 1 | big := 1 ;
        little : { total += 1; count++; } ;
        big : { total += 10; count++; } ;
      endsequence
      return total;
    endfunction
    task timed(string tag);
      randsequence (go)
        go : a b ;
        a : { #5; log = {log, $sformatf("%s@%0t ", tag, $time)}; } ;
        b : { #5; log = {log, $sformatf("%s@%0t ", tag, $time)}; } ;
      endsequence
    endtask
  endclass

  task automatic worker(input string tag, input int delay, output string done);
    done = "";
    randsequence (go)
      go : first second ;
      first : { #(delay); done = {done, tag, "1"}; } ;
      second : { @(posedge clk); done = {done, tag, "2"}; } ;
    endsequence
  endtask

  function automatic int sum(int n);
    int acc;
    randsequence (s)
      s : repeat (n) one ;
      one : { acc += 2; } ;
    endsequence
    return acc;
  endfunction

  always #3 clk = !clk;

  always @(posedge clk) begin
    randsequence (count)
      count : { edges++; } ;
    endsequence
  end

  initial begin
    Gen g;
    int t;
    string x, y;
    g = new;
    t = g.run(4);
    $display("class function: total in range %0d, count %0d", t >= 4 && t <= 40 && (t % 9) == 4,
             g.count);
    $display("function: %0d", sum(5));
    fork
      g.timed("m");
      worker("x", 2, x);
      worker("y", 4, y);
    join
    $display("%0t: class task %s", $time, g.log);
    $display("%0t: tasks %s %s", $time, x, y);
    #20;
    $display("%0t: always block generated %0d times", $time, edges);
    $finish;
  end
endmodule
