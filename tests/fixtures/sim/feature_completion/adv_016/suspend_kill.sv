// A production whose code block waits can be ended by process::kill(),
// disable of an enclosing named block or disable fork (IEEE 1800-2009 9.6,
// 9.7). Nothing after the suspension point runs: no later element, no
// unchosen rule and no weight code block. The suite also runs this fixture
// under AddressSanitizer to show that the saved recursive frames and string
// values of killed generations are released.
module tb;
  timeunit 1ns;
  timeprecision 1ns;
  process p;
  int steps;
  int late;
  string log;

  task automatic recursive_generator(input string tag);
    randsequence (r)
      r : one(steps, tag) r := 1 | one(steps, tag) := 0 ;
      one (int k, string s) : { #1 steps++; log = {log, s}; } tag_it(s) ;
      tag_it (string s) : { #1; } ;
    endsequence
    late++;
  endtask

  initial begin
    // kill() while a recursive generation with saved frames is suspended.
    fork
      begin
        p = process::self();
        recursive_generator("k");
      end
    join_none
    #9 p.kill();
    #3 $display("%0t: killed, steps %0d log %s late %0d", $time, steps, log, late);

    // disable of a named block while a rand join waits. The ratio 0.0
    // takes the shorter sequence first (decision S16-D6), so the order is
    // fixed: right's a, left's a, then left's waiting block.
    log = "";
    fork
      begin : joined
        randsequence (top)
          top : rand join (0.0) left right := 1 { log = {log, "W"}; } ;
          left : a { #10 log = {log, "never"}; } ;
          right : a ;
          a : { #1 log = {log, "a"}; } ;
        endsequence
        late++;
      end
      #4 disable joined;
    join
    $display("%0t: disabled join, log %s late %0d", $time, log, late);

    // disable fork while a weighted production is suspended.
    log = "";
    fork
      randsequence (top)
        top : wait_rule := 1 { log = {log, "W"}; } | other := 0 ;
        wait_rule : { #2 log = {log, "w"}; } { #10 log = {log, "never"}; } ;
        other : { log = {log, "never"}; } ;
      endsequence
    join_none
    #5 disable fork;
    #20 $display("%0t: disable fork, log %s", $time, log);

    // A new generation after the kills starts clean.
    steps = 0;
    log = "";
    fork
      begin
        p = process::self();
        recursive_generator("n");
      end
    join_none
    #4 p.kill();
    $display("%0t: restarted, steps %0d log %s late %0d", $time, steps, log, late);
    $finish;
  end
endmodule
