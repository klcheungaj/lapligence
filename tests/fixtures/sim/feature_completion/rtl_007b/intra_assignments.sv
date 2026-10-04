// Intra-assignment and nonblocking event controls whose helpers have effects
// (SV 9.4.5, 10.4.2, 13.4, 15.5.2).
`timescale 1ns / 1ns
module tb;
  int a = 0, b = 0, c = 0, d = 0, e = 0, g = 0, h = 0, seen = 0;
  int r1 = 0, r2 = 0, q1 = 0, q2 = 0, q3 = 0, q4 = 0;
  int issued = 0, done_at = -1, after_wait = -1;
  logic [3:0] mem [0:3];
  int idx = 1;
  event done;

  // Static function: its formal and the counter are written by every call.
  function int f(input int v);
    seen++;
    return v;
  endfunction

  function logic lsb(input int v);
    seen++;
    return v[0];
  endfunction

  initial begin
    mem[1] = 4'h0;
    mem[2] = 4'h0;
    fork
      r1 = @(f(a)) 11;
      r2 = repeat (2) @(posedge lsb(b)) 22;
      begin
        // The issuing process is not blocked.
        q1 <= @(f(a)) 33;
        issued = 1;
      end
      begin
        // The control is armed at issue: the issuer's own later change is
        // an event.
        q2 <= @(f(c)) 44;
        c = 7;
      end
      begin
        // The destination selector is captured at issue.
        mem[idx] <= @(f(d)) 4'hA;
        idx = 2;
      end
      q4 <= repeat (2) @(negedge lsb(e)) 55;
      ->> @(f(g)) done;
      begin
        @(done);
        done_at = $time;
      end
    join_none
  end

  // A pending process-evaluated NBA is not a child of its issuer.
  initial begin
    q3 <= @(f(h)) 66;
    wait fork;
    disable fork;
    after_wait = $time;
  end

  initial begin
    #1 $display("1: issued=%0d q1=%0d q2=%0d r1=%0d", issued, q1, q2, r1);
    #1 b = 1;
    #1 $display("3: r2=%0d", r2);
    #1 a = 1;
    #1 $display("5: r1=%0d q1=%0d", r1, q1);
    #1 b = 2;
    #1 b = 3;
    #1 $display("8: r2=%0d", r2);
    #1 d = 5;
    #1 $display("10: mem1=%h mem2=%h", mem[1], mem[2]);
    #1 e = 1;
    #1 e = 2;
    #1 e = 3;
    #1 $display("14: q4=%0d", q4);
    #1 e = 4;
    #1 $display("16: q4=%0d", q4);
    #1 g = 1;
    #1 $display("18: done_at=%0d", done_at);
    #1 h = 1;
    #1 $display("20: q3=%0d after_wait=%0d seen_ok=%0d", q3, after_wait, seen >= 10);
    $finish(0);
  end
endmodule
