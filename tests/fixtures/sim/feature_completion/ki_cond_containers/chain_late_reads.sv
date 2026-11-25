// A record alternative is read when the alternatives of its conditional have
// been evaluated, so the last write of a later one is seen; a queue
// alternative is copied when it is evaluated (IEEE 1800-2009 11.4.11).
module tb;
  typedef struct { int a; string t; } rec_t;
  typedef int q_t[$];
  rec_t d0, d4, d8, y, z; q_t q0, qy;
  logic [7:0] s;
  string calls;
  function automatic rec_t f(int k);
    calls = {calls, $sformatf(" f%0d", k)};
    d0.a = 100; d0.t = {d0.t, "m"};
    return '{100, "sm"};
  endfunction
  function automatic q_t qf(int k);
    q0 = '{100 + k};
    return '{0};
  endfunction
  task automatic reset();
    calls = ""; d0 = '{0, "s"}; d4 = '{100, "sm"}; d8 = d4; q0 = '{0};
  endtask
  task automatic go4();
    reset();
    y = s[0] ? d0 : s[1] ? f(1) : s[2] ? f(2) : s[3] ? f(3) : d4;
    qy = s[0] ? q0 : s[1] ? qf(1) : s[2] ? qf(2) : s[3] ? qf(3) : q0;
    $write("4 %b: %0d q %0d%s", s, y.a, qy[0], calls);
    reset();
    z = s[0] ? (s[1] ? (s[2] ? (s[3] ? (f(4)) : f(3)) : f(2)) : f(1)) : f(0);
    $display(" | %0d%s", z.a, calls);
  endtask
  task automatic go8();
    reset();
    y = s[0] ? d0 : s[1] ? f(1) : s[2] ? f(2) : s[3] ? f(3) : s[4] ? f(4) : s[5] ? f(5) : s[6] ? f(6) : s[7]
          ? f(7) : d8;
    qy = s[0] ? q0 : s[1] ? qf(1) : s[2] ? qf(2) : s[3] ? qf(3) : s[4] ? qf(4) : s[5] ? qf(5) : s[6] ? qf(6)
          : s[7] ? qf(7) : q0;
    $write("8 %b: %0d q %0d%s", s, y.a, qy[0], calls);
    reset();
    z = s[0] ? (s[1] ? (s[2] ? (s[3] ? (s[4] ? (s[5] ? (s[6] ? (s[7] ? (f(8)) : f(7)) : f(6)) : f(5)) : f(4))
          : f(3)) : f(2)) : f(1)) : f(0);
    $display(" | %0d%s", z.a, calls);
  endtask
  initial begin
    for (int k = 0; k < 256; k++) begin
      s = k; if (k % 3 == 0) s[0] = 1'bx;
      if (k % 5 == 0) s[2] = 1'bz; if (k % 7 == 0) s[1] = 1'bx; if (k % 11 == 0) s[6] = 1'bx;
      if (k < 16) go4();
      if (k % 4 != 1) go8();
    end
  end
endmodule
