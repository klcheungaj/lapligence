// Decision S07-D14: a nonblocking trigger `->> ev[k]` of a missing entry of an
// associative array of events creates nothing and triggers nothing. It
// resolves the event when issued (SV 15.5.1), and only an event control or a
// blocking trigger creates a missing entry (S07-D11); an entry that exists
// when the trigger is issued is triggered in the NBA region.
//
// IEEE 1800-2009 15.5.1 (SystemVerilog-1800-2009.txt L20708-20710): "The
// effect of the ->> operator is that the statement executes without blocking
// and it creates a nonblocking assign update event in the time in which the
// delay control expires or the event control occurs." 7.8.6 (L8720, L8735)
// gives the value a read of a nonexistent entry returns; the text is silent on
// a trigger of one. llg follows industry-standard behaviour.
module tb;
  event ev[int];
  initial begin
    ->> ev[3];
    $display("issue %0d", ev.exists(3));
    #0 $display("inactive %0d", ev.exists(3));
    fork
      begin @(ev[3]); $display("late waiter woke %0d", $time); end
    join_none
    #1 $display("t1 %0d %0d", ev.exists(3), ev.num());
    -> ev[3];
    #1 ->> #2 ev[7];
    #3 $display("delayed %0d", ev.exists(7));
    ->> ev[9];
    ->> ev[9];
    #1 $display("twice %0d %0d", ev.exists(9), ev.num());
  end
  initial begin
    @(ev[5]) $display("early waiter woke %0d %0d", ev.exists(5), $time);
  end
  initial begin
    ->> ev[5];
    $display("existing %0d", ev.exists(5));
  end
endmodule
