// Decision T1-D8: an event on a net whose strength can change without its value
// also runs on a strength-only change. A view takes eight bits per net bit, so
// a net past an eighth of the packed width limit cannot have one. Such a reader
// rejects: waking on values alone would lose those runs without notice.
module tb;
  tri1 [131071:0] w;
  always @(w) $display("woke");
  initial #1 $finish;
endmodule
