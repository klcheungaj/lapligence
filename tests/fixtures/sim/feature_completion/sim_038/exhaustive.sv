// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/exhaustive.sv
// SIM-038 A01: every property below starts on tick 1 of each of the 1024
// two-signal traces of length 5 (go at tick 0). The assert prints its pass
// (A) and fail (F) actions, the cover of the same property prints only
// nonvacuous successes (C), and pending attempts are disabled between traces.
// The expected lines come from the test-side Annex F property interpreter in
// sim_038.rs.
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic kill = 1'b1;
  logic a = 1'b0;
  logic b = 1'b0;
  int trace = 0;
  int pos = 0;

  p00: assert property (@(posedge clk) disable iff (kill) go |=> a) $display("A00 %0d %0d", trace, pos); else $display("F00 %0d %0d", trace, pos);
  c00: cover property (@(posedge clk) disable iff (kill) go |=> a) $display("C00 %0d %0d", trace, pos);
  p01: assert property (@(posedge clk) disable iff (kill) go |=> strong(a ##1 b)) $display("A01 %0d %0d", trace, pos); else $display("F01 %0d %0d", trace, pos);
  c01: cover property (@(posedge clk) disable iff (kill) go |=> strong(a ##1 b)) $display("C01 %0d %0d", trace, pos);
  p02: assert property (@(posedge clk) disable iff (kill) go |=> weak(a ##[1:2] b)) $display("A02 %0d %0d", trace, pos); else $display("F02 %0d %0d", trace, pos);
  c02: cover property (@(posedge clk) disable iff (kill) go |=> weak(a ##[1:2] b)) $display("C02 %0d %0d", trace, pos);
  p03: assert property (@(posedge clk) disable iff (kill) go |=> (not (a ##1 b))) $display("A03 %0d %0d", trace, pos); else $display("F03 %0d %0d", trace, pos);
  c03: cover property (@(posedge clk) disable iff (kill) go |=> (not (a ##1 b))) $display("C03 %0d %0d", trace, pos);
  p04: assert property (@(posedge clk) disable iff (kill) go |=> (not strong(a ##[0:1] b))) $display("A04 %0d %0d", trace, pos); else $display("F04 %0d %0d", trace, pos);
  c04: cover property (@(posedge clk) disable iff (kill) go |=> (not strong(a ##[0:1] b))) $display("C04 %0d %0d", trace, pos);
  p05: assert property (@(posedge clk) disable iff (kill) go |=> ((a ##1 b) or (nexttime a))) $display("A05 %0d %0d", trace, pos); else $display("F05 %0d %0d", trace, pos);
  c05: cover property (@(posedge clk) disable iff (kill) go |=> ((a ##1 b) or (nexttime a))) $display("C05 %0d %0d", trace, pos);
  p06: assert property (@(posedge clk) disable iff (kill) go |=> ((a |-> (nexttime b)) and (b |-> (nexttime a)))) $display("A06 %0d %0d", trace, pos); else $display("F06 %0d %0d", trace, pos);
  c06: cover property (@(posedge clk) disable iff (kill) go |=> ((a |-> (nexttime b)) and (b |-> (nexttime a)))) $display("C06 %0d %0d", trace, pos);
  p07: assert property (@(posedge clk) disable iff (kill) go |=> (if (a) (nexttime b))) $display("A07 %0d %0d", trace, pos); else $display("F07 %0d %0d", trace, pos);
  c07: cover property (@(posedge clk) disable iff (kill) go |=> (if (a) (nexttime b))) $display("C07 %0d %0d", trace, pos);
  p08: assert property (@(posedge clk) disable iff (kill) go |=> (if (a) (nexttime b) else (always [0:1] !b))) $display("A08 %0d %0d", trace, pos); else $display("F08 %0d %0d", trace, pos);
  c08: cover property (@(posedge clk) disable iff (kill) go |=> (if (a) (nexttime b) else (always [0:1] !b))) $display("C08 %0d %0d", trace, pos);
  p09: assert property (@(posedge clk) disable iff (kill) go |=> (case (a) 1'b1: (nexttime b); default: (always [0:1] !b); endcase)) $display("A09 %0d %0d", trace, pos); else $display("F09 %0d %0d", trace, pos);
  c09: cover property (@(posedge clk) disable iff (kill) go |=> (case (a) 1'b1: (nexttime b); default: (always [0:1] !b); endcase)) $display("C09 %0d %0d", trace, pos);
  p10: assert property (@(posedge clk) disable iff (kill) go |=> (a |-> (b until a))) $display("A10 %0d %0d", trace, pos); else $display("F10 %0d %0d", trace, pos);
  c10: cover property (@(posedge clk) disable iff (kill) go |=> (a |-> (b until a))) $display("C10 %0d %0d", trace, pos);
  p11: assert property (@(posedge clk) disable iff (kill) go |=> ((a ##1 b) #-# (nexttime a))) $display("A11 %0d %0d", trace, pos); else $display("F11 %0d %0d", trace, pos);
  c11: cover property (@(posedge clk) disable iff (kill) go |=> ((a ##1 b) #-# (nexttime a))) $display("C11 %0d %0d", trace, pos);
  p12: assert property (@(posedge clk) disable iff (kill) go |=> (a #=# (b until_with a))) $display("A12 %0d %0d", trace, pos); else $display("F12 %0d %0d", trace, pos);
  c12: cover property (@(posedge clk) disable iff (kill) go |=> (a #=# (b until_with a))) $display("C12 %0d %0d", trace, pos);
  p13: assert property (@(posedge clk) disable iff (kill) go |=> ((nexttime a) implies (b ##1 b))) $display("A13 %0d %0d", trace, pos); else $display("F13 %0d %0d", trace, pos);
  c13: cover property (@(posedge clk) disable iff (kill) go |=> ((nexttime a) implies (b ##1 b))) $display("C13 %0d %0d", trace, pos);
  p14: assert property (@(posedge clk) disable iff (kill) go |=> ((nexttime a) iff b)) $display("A14 %0d %0d", trace, pos); else $display("F14 %0d %0d", trace, pos);
  c14: cover property (@(posedge clk) disable iff (kill) go |=> ((nexttime a) iff b)) $display("C14 %0d %0d", trace, pos);
  p15: assert property (@(posedge clk) disable iff (kill) go |=> (nexttime [2] a)) $display("A15 %0d %0d", trace, pos); else $display("F15 %0d %0d", trace, pos);
  c15: cover property (@(posedge clk) disable iff (kill) go |=> (nexttime [2] a)) $display("C15 %0d %0d", trace, pos);
  p16: assert property (@(posedge clk) disable iff (kill) go |=> (s_nexttime a)) $display("A16 %0d %0d", trace, pos); else $display("F16 %0d %0d", trace, pos);
  c16: cover property (@(posedge clk) disable iff (kill) go |=> (s_nexttime a)) $display("C16 %0d %0d", trace, pos);
  p17: assert property (@(posedge clk) disable iff (kill) go |=> (s_nexttime [2] (a && b))) $display("A17 %0d %0d", trace, pos); else $display("F17 %0d %0d", trace, pos);
  c17: cover property (@(posedge clk) disable iff (kill) go |=> (s_nexttime [2] (a && b))) $display("C17 %0d %0d", trace, pos);
  p18: assert property (@(posedge clk) disable iff (kill) go |=> (nexttime [0] b)) $display("A18 %0d %0d", trace, pos); else $display("F18 %0d %0d", trace, pos);
  c18: cover property (@(posedge clk) disable iff (kill) go |=> (nexttime [0] b)) $display("C18 %0d %0d", trace, pos);
  p19: assert property (@(posedge clk) disable iff (kill) go |=> (always [1:3] a)) $display("A19 %0d %0d", trace, pos); else $display("F19 %0d %0d", trace, pos);
  c19: cover property (@(posedge clk) disable iff (kill) go |=> (always [1:3] a)) $display("C19 %0d %0d", trace, pos);
  p20: assert property (@(posedge clk) disable iff (kill) go |=> (s_always [0:2] b)) $display("A20 %0d %0d", trace, pos); else $display("F20 %0d %0d", trace, pos);
  c20: cover property (@(posedge clk) disable iff (kill) go |=> (s_always [0:2] b)) $display("C20 %0d %0d", trace, pos);
  p21: assert property (@(posedge clk) disable iff (kill) go |=> (always a)) $display("A21 %0d %0d", trace, pos); else $display("F21 %0d %0d", trace, pos);
  c21: cover property (@(posedge clk) disable iff (kill) go |=> (always a)) $display("C21 %0d %0d", trace, pos);
  p22: assert property (@(posedge clk) disable iff (kill) go |=> (always [2:$] a)) $display("A22 %0d %0d", trace, pos); else $display("F22 %0d %0d", trace, pos);
  c22: cover property (@(posedge clk) disable iff (kill) go |=> (always [2:$] a)) $display("C22 %0d %0d", trace, pos);
  p23: assert property (@(posedge clk) disable iff (kill) go |=> (s_eventually a)) $display("A23 %0d %0d", trace, pos); else $display("F23 %0d %0d", trace, pos);
  c23: cover property (@(posedge clk) disable iff (kill) go |=> (s_eventually a)) $display("C23 %0d %0d", trace, pos);
  p24: assert property (@(posedge clk) disable iff (kill) go |=> (eventually [1:2] b)) $display("A24 %0d %0d", trace, pos); else $display("F24 %0d %0d", trace, pos);
  c24: cover property (@(posedge clk) disable iff (kill) go |=> (eventually [1:2] b)) $display("C24 %0d %0d", trace, pos);
  p25: assert property (@(posedge clk) disable iff (kill) go |=> (s_eventually [1:3] (a && b))) $display("A25 %0d %0d", trace, pos); else $display("F25 %0d %0d", trace, pos);
  c25: cover property (@(posedge clk) disable iff (kill) go |=> (s_eventually [1:3] (a && b))) $display("C25 %0d %0d", trace, pos);
  p26: assert property (@(posedge clk) disable iff (kill) go |=> (s_eventually [2:$] b)) $display("A26 %0d %0d", trace, pos); else $display("F26 %0d %0d", trace, pos);
  c26: cover property (@(posedge clk) disable iff (kill) go |=> (s_eventually [2:$] b)) $display("C26 %0d %0d", trace, pos);
  p27: assert property (@(posedge clk) disable iff (kill) go |=> (a until b)) $display("A27 %0d %0d", trace, pos); else $display("F27 %0d %0d", trace, pos);
  c27: cover property (@(posedge clk) disable iff (kill) go |=> (a until b)) $display("C27 %0d %0d", trace, pos);
  p28: assert property (@(posedge clk) disable iff (kill) go |=> (a s_until b)) $display("A28 %0d %0d", trace, pos); else $display("F28 %0d %0d", trace, pos);
  c28: cover property (@(posedge clk) disable iff (kill) go |=> (a s_until b)) $display("C28 %0d %0d", trace, pos);
  p29: assert property (@(posedge clk) disable iff (kill) go |=> (a until_with b)) $display("A29 %0d %0d", trace, pos); else $display("F29 %0d %0d", trace, pos);
  c29: cover property (@(posedge clk) disable iff (kill) go |=> (a until_with b)) $display("C29 %0d %0d", trace, pos);
  p30: assert property (@(posedge clk) disable iff (kill) go |=> (a s_until_with b)) $display("A30 %0d %0d", trace, pos); else $display("F30 %0d %0d", trace, pos);
  c30: cover property (@(posedge clk) disable iff (kill) go |=> (a s_until_with b)) $display("C30 %0d %0d", trace, pos);
  p31: assert property (@(posedge clk) disable iff (kill) go |=> (accept_on (b) (always a))) $display("A31 %0d %0d", trace, pos); else $display("F31 %0d %0d", trace, pos);
  c31: cover property (@(posedge clk) disable iff (kill) go |=> (accept_on (b) (always a))) $display("C31 %0d %0d", trace, pos);
  p32: assert property (@(posedge clk) disable iff (kill) go |=> (reject_on (b) (a ##[1:3] a))) $display("A32 %0d %0d", trace, pos); else $display("F32 %0d %0d", trace, pos);
  c32: cover property (@(posedge clk) disable iff (kill) go |=> (reject_on (b) (a ##[1:3] a))) $display("C32 %0d %0d", trace, pos);
  p33: assert property (@(posedge clk) disable iff (kill) go |=> (sync_accept_on ((a && b)) (b |=> (always [0:2] !a)))) $display("A33 %0d %0d", trace, pos); else $display("F33 %0d %0d", trace, pos);
  c33: cover property (@(posedge clk) disable iff (kill) go |=> (sync_accept_on ((a && b)) (b |=> (always [0:2] !a)))) $display("C33 %0d %0d", trace, pos);
  p34: assert property (@(posedge clk) disable iff (kill) go |=> (sync_reject_on (!a) (a ##2 b))) $display("A34 %0d %0d", trace, pos); else $display("F34 %0d %0d", trace, pos);
  c34: cover property (@(posedge clk) disable iff (kill) go |=> (sync_reject_on (!a) (a ##2 b))) $display("C34 %0d %0d", trace, pos);
  p35: assert property (@(posedge clk) disable iff (kill) go |=> (always [0:2] (a |-> (nexttime b)))) $display("A35 %0d %0d", trace, pos); else $display("F35 %0d %0d", trace, pos);
  c35: cover property (@(posedge clk) disable iff (kill) go |=> (always [0:2] (a |-> (nexttime b)))) $display("C35 %0d %0d", trace, pos);
  p36: assert property (@(posedge clk) disable iff (kill) go |=> (s_eventually [0:2] (a and (nexttime a)))) $display("A36 %0d %0d", trace, pos); else $display("F36 %0d %0d", trace, pos);
  c36: cover property (@(posedge clk) disable iff (kill) go |=> (s_eventually [0:2] (a and (nexttime a)))) $display("C36 %0d %0d", trace, pos);
  p37: assert property (@(posedge clk) disable iff (kill) go |=> (not (a until b))) $display("A37 %0d %0d", trace, pos); else $display("F37 %0d %0d", trace, pos);
  c37: cover property (@(posedge clk) disable iff (kill) go |=> (not (a until b))) $display("C37 %0d %0d", trace, pos);
  p38: assert property (@(posedge clk) disable iff (kill) go |=> ((always [0:1] a) or (s_eventually [0:2] b))) $display("A38 %0d %0d", trace, pos); else $display("F38 %0d %0d", trace, pos);
  c38: cover property (@(posedge clk) disable iff (kill) go |=> ((always [0:1] a) or (s_eventually [0:2] b))) $display("C38 %0d %0d", trace, pos);
  p39: assert property (@(posedge clk) disable iff (kill) go |=> (a |=> (b implies (s_nexttime a)))) $display("A39 %0d %0d", trace, pos); else $display("F39 %0d %0d", trace, pos);
  c39: cover property (@(posedge clk) disable iff (kill) go |=> (a |=> (b implies (s_nexttime a)))) $display("C39 %0d %0d", trace, pos);
  p40: assert property (@(posedge clk) disable iff (kill) go |=> (accept_on ((a && b)) (b |=> (always [0:2] !a)))) $display("A40 %0d %0d", trace, pos); else $display("F40 %0d %0d", trace, pos);
  c40: cover property (@(posedge clk) disable iff (kill) go |=> (accept_on ((a && b)) (b |=> (always [0:2] !a)))) $display("C40 %0d %0d", trace, pos);
  p41: assert property (@(posedge clk) disable iff (kill) go |=> (not (s_eventually [1:2] a))) $display("A41 %0d %0d", trace, pos); else $display("F41 %0d %0d", trace, pos);
  c41: cover property (@(posedge clk) disable iff (kill) go |=> (not (s_eventually [1:2] a))) $display("C41 %0d %0d", trace, pos);
  p42: assert property (@(posedge clk) disable iff (kill) go |=> ((a ##[0:1] b) |-> (nexttime a))) $display("A42 %0d %0d", trace, pos); else $display("F42 %0d %0d", trace, pos);
  c42: cover property (@(posedge clk) disable iff (kill) go |=> ((a ##[0:1] b) |-> (nexttime a))) $display("C42 %0d %0d", trace, pos);
  p43: assert property (@(posedge clk) disable iff (kill) go |=> (always [0:3] (b or (nexttime b)))) $display("A43 %0d %0d", trace, pos); else $display("F43 %0d %0d", trace, pos);
  c43: cover property (@(posedge clk) disable iff (kill) go |=> (always [0:3] (b or (nexttime b)))) $display("C43 %0d %0d", trace, pos);
  p44: assert property (@(posedge clk) disable iff (kill) go |=> ((a ##1 a) s_until b)) $display("A44 %0d %0d", trace, pos); else $display("F44 %0d %0d", trace, pos);
  c44: cover property (@(posedge clk) disable iff (kill) go |=> ((a ##1 a) s_until b)) $display("C44 %0d %0d", trace, pos);

  task automatic tick();
    #5 clk = 1'b1;
    #5 clk = 1'b0;
  endtask

  initial begin
    for (int tr = 0; tr < 1024; tr++) begin
      trace = tr;
      pos = 0;
      go = 1'b1;
      kill = 1'b0;
      a = 1'b0;
      b = 1'b0;
      tick();
      go = 1'b0;
      for (int k = 0; k < 5; k++) begin
        pos = k + 1;
        a = tr[2 * k];
        b = tr[2 * k + 1];
        tick();
      end
      kill = 1'b1;
      tick();
    end
    $finish;
  end
endmodule
