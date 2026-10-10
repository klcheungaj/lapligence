// Decision S36-D2: which resumptions are deferred assertion flush points.
//
// IEEE 1800-2009 16.4.2 (SystemVerilog-1800-2009.txt L21276-21281):
//   "A process is defined to have reached a deferred assertion flush point if
//   any of the following occur:
//   - The process, having been suspended earlier due to reaching an event
//     control or wait statement, resumes execution.
//   - The process was declared by an always_comb or always_latch, and its
//     execution is resumed due to a transition on one of its dependent
//     signals.
//   - The outermost scope of the process is disabled by a disable statement"
//
// llg treats resumption after `wait (...)`, `@(...)`, `wait fork` and
// `wait_order` as flush points, and resumption after a delay control (`#0`,
// `#n`), a fork-join, or a blocking built-in method (semaphore, mailbox,
// process await) as not a flush point, because those are not event controls
// or wait statements.
module tb;
  logic go = 1'b0, ev = 1'b0;
  initial begin
    d1: assert #0 (1'b0) else $display("kept across #0");
    #0;
    #1;
    w1: assert #0 (1'b0) else $display("not printed: flushed by wait");
    wait (go);
    e1: assert #0 (1'b0) else $display("not printed: flushed by event control");
    @(ev);
    $display("resumed");
    #1 $finish;
  end
  initial begin
    #1;
    #0 go = 1'b1;
    #0 ev = 1'b1;
  end
endmodule
