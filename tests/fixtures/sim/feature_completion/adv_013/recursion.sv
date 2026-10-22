// ADV-013 A02: recursive properties (16.13.17, Annex F.7): the 16.13.17
// examples (always and weak until through recursion, mutual recursion, the
// accept/reject example in its synchronous form), swapped and constant
// actuals, two recursive call sites, recursion under a second clock and the
// F.7 dependency example. Same stimulus as flow.sv; expected lines are
// derived in readme.md.
`timescale 1ns/1ps
module tb;
  logic c1 = 0, c2 = 0, a = 0, b = 0, d = 0, e = 0;
  initial begin
    fork
      begin #10 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; #15 c1 = 1; #5 c1 = 0; end
      begin #20 c2 = 1; #5 c2 = 0; #5 c2 = 1; #3 c2 = 0; #12 c2 = 1; #3 c2 = 0; #12 c2 = 1; #3 c2 = 0; #27 c2 = 1; #2 c2 = 0; #8 c2 = 1; #2 c2 = 0; end
      begin #1 a = 1; #11 a = 0; #10 a = 1; #19 a = 0; #20 a = 1; end
      begin #2 b = 1; #19 b = 0; #10 b = 1; #20 b = 0; #7 b = 1; #33 b = 0; end
      begin #3 d = 1; #41 d = 0; #22 d = 1; end
      begin #26 e = 1; #10 e = 0; end
    join
    #13 $finish;
  end

  property prop_always(p);
    p and (1'b1 |=> prop_always(p));
  endproperty
  property prop_weak_until(p, q);
    q or (p and (1'b1 |=> prop_weak_until(p, q)));
  endproperty
  property check_phase1;
    a |-> (d and (1'b1 |=> check_phase2));
  endproperty
  property check_phase2;
    b |-> (d and (1'b1 |=> check_phase1));
  endproperty
  property swap(x, y);
    x and (1'b1 |=> swap(y, x));
  endproperty
  property then_d(x);
    x and (1'b1 |=> then_d(d));
  endproperty
  property two_sites(x, y);
    (x |=> (y and two_sites(x, y))) and (y or (1'b1 |=> two_sites(x, y)));
  endproperty
  property sync_p3(p, bit bb, abort);
    (p and (1'b1 |=> sync_p4(p, bb, abort)));
  endproperty
  property sync_p4(p, bit bb, abort);
    sync_accept_on(bb) sync_reject_on(abort) sync_p3(p, bb, abort);
  endproperty
  property on_c2(x);
    x and (1'b1 |=> @(posedge c2) on_c2(x));
  endproperty
  property p1(v);
    v |=> p2(p3());
  endproperty
  property p2(v);
    a or (1'b1 |=> v);
  endproperty
  property p3;
    p1(a && b);
  endproperty

  r1: assert property (@(posedge c1) a |-> prop_always(a)) $display("r1 P %0t", $time); else $display("r1 F %0t", $time);
  r2: assert property (@(posedge c1) a |=> prop_always(d)) $display("r2 P %0t", $time); else $display("r2 F %0t", $time);
  r3: assert property (@(posedge c1) prop_weak_until(d, e)) $display("r3 P %0t", $time); else $display("r3 F %0t", $time);
  r4: assert property (@(posedge c1) check_phase1) $display("r4 P %0t", $time); else $display("r4 F %0t", $time);
  r5: assert property (@(posedge c1) a |=> swap(b, d)) $display("r5 P %0t", $time); else $display("r5 F %0t", $time);
  r6: assert property (@(posedge c1) a |=> then_d(e)) $display("r6 P %0t", $time); else $display("r6 F %0t", $time);
  r7: assert property (@(posedge c1) two_sites(e, b)) $display("r7 P %0t", $time); else $display("r7 F %0t", $time);
  r8: assert property (@(posedge c1) a |-> sync_p3(d, b, e)) $display("r8 P %0t", $time); else $display("r8 F %0t", $time);
  r9: assert property (@(posedge c1) a |=> @(posedge c2) prop_always(d)) $display("r9 P %0t", $time); else $display("r9 F %0t", $time);
  r10: assert property (@(posedge c1) a |=> on_c2(d)) $display("r10 P %0t", $time); else $display("r10 F %0t", $time);
  r11: assert property (@(posedge c1) p1(d)) $display("r11 P %0t", $time); else $display("r11 F %0t", $time);
endmodule
