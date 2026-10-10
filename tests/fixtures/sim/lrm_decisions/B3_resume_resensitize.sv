// B3: resume() resensitizes a process that was suspended while blocked on an
// event control; an event that occurs while it is suspended is not delivered.
//
// IEEE 1800-2009 9.7 (L12644-12647): "Calling resume on a process that was
// suspended while blocked on another condition shall resensitize the process
// to the event expression or to wait for the wait condition to become true or
// for the delay to expire. If the wait condition is now true or the original
// delay has transpired, the process is scheduled onto the Active or Reactive
// region to continue its execution in the current time step."
//
// Decision: @e and @(posedge clk) waiters stay WAITING after resume() and
// need a new occurrence; a wait condition that became true while suspended
// completes in the time step of resume().
`timescale 1ns / 1ns
module tb;
  event e;
  logic clk;
  int v;
  process p;

  initial begin
    clk = 0;
    v = 0;
    fork
      begin
        p = process::self();
        @e;
        $display("%0t: @e resumed", $time);
      end
    join_none
    #1 p.suspend();
    #1 ->e;
    #1 p.resume();
    #1 $display("%0t: after resume the @e waiter is %s", $time, p.status().name());
    ->e;
    #1;
    fork
      begin
        p = process::self();
        @(posedge clk);
        $display("%0t: @(posedge clk) resumed", $time);
      end
    join_none
    #1 p.suspend();
    #1 clk = 1;
    #1 p.resume();
    #1 $display("%0t: after resume the @(posedge clk) waiter is %s", $time, p.status().name());
    clk = 0;
    #1 clk = 1;
    #1;
    fork
      begin
        p = process::self();
        wait (v == 1);
        $display("%0t: wait (v == 1) resumed", $time);
      end
    join_none
    #1 p.suspend();
    #1 v = 1;
    #1 p.resume();
    #1 $display("%0t: done", $time);
    $finish;
  end
endmodule
