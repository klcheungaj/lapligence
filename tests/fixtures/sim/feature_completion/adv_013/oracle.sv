// ADV-013 A01: randomized traces for the test-side Annex F interpreter
// (tests/sim_feature_completion/adv_013/oracle.rs). Each `unit` instance
// draws its own trace from a xorshift32 generator seeded by SEED: every 10
// time units, a and b change at offset 4 and posedge c1 / posedge c2 occur
// at offset 5 when bits 2 / 3 of the state are set, so the clocks tick
// irregularly, sometimes together and sometimes not at all. Every assertion
// line is generated from the interpreter's property list; the suite checks
// that each line appears here. The expected output is computed by the
// interpreter, not stored.
`timescale 1ns / 1ns
module unit #(parameter int unsigned SEED = 1) ();
  logic c1 = 1'b0, c2 = 1'b0, a = 1'b0, b = 1'b0;
  int unsigned s = SEED;
  initial begin
    repeat (16) begin
      #4;
      s = s ^ (s << 13);
      s = s ^ (s >> 17);
      s = s ^ (s << 5);
      a = s[0];
      b = s[1];
      #1 begin
        c1 = s[2];
        c2 = s[3];
      end
      #5 begin
        c1 = 1'b0;
        c2 = 1'b0;
      end
    end
  end

  property ra(x);
    x and (1'b1 |=> ra(x));
  endproperty
  property wu(p, q);
    q or (p and (1'b1 |=> wu(p, q)));
  endproperty
  property oc2(x);
    x and (1'b1 |=> @(posedge c2) oc2(x));
  endproperty
  property ph1;
    a |-> (b and (1'b1 |=> ph2));
  endproperty
  property ph2;
    b |-> (a and (1'b1 |=> ph1));
  endproperty
  property ts(x, y);
    (x |=> (y and ts(x, y))) and (y or (1'b1 |=> ts(x, y)));
  endproperty
  p00: assert property (@(posedge c1) a ##1 @(posedge c2) b) $display("P00 %0d %0t", SEED, $time); else $display("F00 %0d %0t", SEED, $time);
  p01: assert property (@(posedge c1) a ##0 @(posedge c2) b) $display("P01 %0d %0t", SEED, $time); else $display("F01 %0d %0t", SEED, $time);
  p02: assert property (@(posedge c1) a |=> @(posedge c2) b) $display("P02 %0d %0t", SEED, $time); else $display("F02 %0d %0t", SEED, $time);
  p03: assert property (@(posedge c1) a |-> @(posedge c2) b ##1 a) $display("P03 %0d %0t", SEED, $time); else $display("F03 %0d %0t", SEED, $time);
  p04: assert property (@(posedge c1) a ##1 @(posedge c2) b |=> a) $display("P04 %0d %0t", SEED, $time); else $display("F04 %0d %0t", SEED, $time);
  p05: assert property (@(posedge c1) (a |=> @(posedge c2) b) and (b |=> a)) $display("P05 %0d %0t", SEED, $time); else $display("F05 %0d %0t", SEED, $time);
  p06: assert property (@(posedge c1) a |=> (@(posedge c2) b) or (@(posedge c1) a ##1 b)) $display("P06 %0d %0t", SEED, $time); else $display("F06 %0d %0t", SEED, $time);
  p07: assert property (@(posedge c1) not (a ##1 @(posedge c2) b)) $display("P07 %0d %0t", SEED, $time); else $display("F07 %0d %0t", SEED, $time);
  p08: assert property (@(posedge c1) if (a) @(posedge c2) b else @(posedge c2) !b) $display("P08 %0d %0t", SEED, $time); else $display("F08 %0d %0t", SEED, $time);
  p09: assert property (@(posedge c1) a |=> nexttime (@(posedge c2) b)) $display("P09 %0d %0t", SEED, $time); else $display("F09 %0d %0t", SEED, $time);
  p10: assert property (@(posedge c1) a |=> ((@(posedge c2) b) until a)) $display("P10 %0d %0t", SEED, $time); else $display("F10 %0d %0t", SEED, $time);
  p11: assert property (@(posedge c1) a |=> always (@(posedge c2) b)) $display("P11 %0d %0t", SEED, $time); else $display("F11 %0d %0t", SEED, $time);
  p12: assert property (@(posedge c1) a |=> @(posedge c2) ra(b)) $display("P12 %0d %0t", SEED, $time); else $display("F12 %0d %0t", SEED, $time);
  p13: assert property (@(posedge c1) wu(a, b)) $display("P13 %0d %0t", SEED, $time); else $display("F13 %0d %0t", SEED, $time);
  p14: assert property (@(posedge c1) a |=> oc2(b)) $display("P14 %0d %0t", SEED, $time); else $display("F14 %0d %0t", SEED, $time);
  p15: assert property (@(posedge c1) ph1) $display("P15 %0d %0t", SEED, $time); else $display("F15 %0d %0t", SEED, $time);
  p16: assert property (@(posedge c1) ts(a, b)) $display("P16 %0d %0t", SEED, $time); else $display("F16 %0d %0t", SEED, $time);
  p17: assert property (@(posedge c1) a |=> strong(@(posedge c2) b ##1 b)) $display("P17 %0d %0t", SEED, $time); else $display("F17 %0d %0t", SEED, $time);
  p18: assert property (@(posedge c1) b |-> sync_accept_on(a) (1'b1 |=> @(posedge c2) b)) $display("P18 %0d %0t", SEED, $time); else $display("F18 %0d %0t", SEED, $time);
endmodule

module tb;
  unit #(.SEED(32'd2654435761)) u00 ();
  unit #(.SEED(32'd1013904227)) u01 ();
  unit #(.SEED(32'd3668339987)) u02 ();
  unit #(.SEED(32'd2027808453)) u03 ();
  unit #(.SEED(32'd387276917)) u04 ();
  unit #(.SEED(32'd3041712679)) u05 ();
  unit #(.SEED(32'd1401181143)) u06 ();
  unit #(.SEED(32'd4055616905)) u07 ();
  unit #(.SEED(32'd2415085369)) u08 ();
  unit #(.SEED(32'd774553835)) u09 ();
  unit #(.SEED(32'd3428989595)) u10 ();
  unit #(.SEED(32'd1788458061)) u11 ();
  initial #170 $finish;
endmodule
