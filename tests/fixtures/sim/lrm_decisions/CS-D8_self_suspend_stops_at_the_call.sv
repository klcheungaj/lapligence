// Decision CS-D8: a process that suspends itself stops at the call to
// suspend(): the statements after it run only when another process resumes
// it, at the time of the resume; until then its status reads SUSPENDED.
//
// IEEE 1800-2009 9.7 (SystemVerilog-1800-2009.txt L12639-12641):
//   "The suspend() function allows a process to suspend either its own execution or that of another process. If
//   the process to be suspended is not blocked waiting on some other condition, such as an event, wait expres-
//   sion, or a delay, then the process shall be suspended at some unspecified time in the current time step."
// L12648-12649:
//   "Calling resume on a process that suspends itself causes the process to continue to exe-
//   cute at the statement following the call to suspend."
// An industry-standard simulator runs a process that suspends itself on to
// its next blocking statement and suspends it there: `p` prints `resumed` at
// 0 and sets n, `q` runs to its end (FINISHED at 1), and the resume at 1
// continues `p` at its delay. The text resumes such a process at the
// statement after the call, which the statements it already ran contradict;
// llg suspends at the call.
`timescale 1ns/1ns
module tb;
  process p, q;
  int n;
  initial begin
    fork
      begin
        p = process::self();
        p.suspend();
        n = 1;
        $display("[%0t] p resumed st=%s", $time, p.status().name());
        #3 $display("[%0t] p after #3", $time);
      end
      begin
        q = process::self();
        q.suspend();
        $display("[%0t] q resumed", $time);
      end
    join_none
    #1 $display("[%0t] p=%s q=%s n=%0d", $time, p.status().name(), q.status().name(), n);
    p.resume();
    #0 $display("[%0t] after resume p=%s n=%0d", $time, p.status().name(), n);
    #1 q.resume();
    #0 $display("[%0t] q=%s", $time, q.status().name());
    #5 $display("[%0t] p=%s", $time, p.status().name());
    $finish(0);
  end
endmodule
