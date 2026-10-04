// SIM-004 boundary: a static subroutine record with a string leaf is a legal
// nonblocking target (IEEE 1800-2009 10.4.2), but its storage is a native
// root whose leaves move when the root is replaced, so a queued leaf pointer
// cannot be retained. It is rejected explicitly rather than queued unsafely.
module tb;
  typedef struct {string s; int n;} rec_t;
  rec_t seen;
  task static keep();
    rec_t held;
    held <= '{"a", 1};
    #1 seen = held;
  endtask
  initial begin
    keep();
    $display("%s", seen.s);
    $finish(0);
  end
endmodule
