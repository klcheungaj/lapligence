// SIM-017: processes woken by a delivered message re-enter the same mailbox.
// The receiver writes its destination when it resumes; an `always @(dest)`
// reacting to that write puts again, which is handed straight to the
// receiver blocked in its next get, and a continuous assignment reading the
// destination peeks the (empty) mailbox. Every message is received exactly
// once (SV 15.4, 9.4.2, 10.3).
module tb;
  mailbox #(int) m = new();
  int dest;
  int echoes[$];
  int peeked;

  function automatic int peek_next(int unused);
    int t;
    if (m.try_peek(t) > 0) return t;
    return -1;
  endfunction

  assign peeked = peek_next(dest);

  always @(dest) begin
    if (dest < 100) void'(m.try_put(dest + 100));
  end

  initial begin
    fork
      repeat (4) begin
        m.get(dest);
        echoes.push_back(dest);
      end
    join_none
    #1 m.put(1);
    #1 m.put(2);
    #1;
    $display("echoes %0d: %0d %0d %0d %0d", echoes.size(), echoes[0], echoes[1], echoes[2],
             echoes[3]);
    $display("dest=%0d peeked=%0d n=%0d", dest, peeked, m.num());
    m.put(3);
    $display("left n=%0d", m.num());
    $finish;
  end
endmodule
