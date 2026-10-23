// $nochange with negative offsets (31.4.6) and with a delayed reference
// signal (31.9.1). The a, b and e instances share c, which falls (leading
// edge) at 100 and rises (trailing edge) at 120; the g instances read C,
// which rises at 200 and falls at 220 and is delayed by 6 by the $setuphold
// of their module when negative timing checks are enabled. Each checked
// module counts its notifier changes and keeps the time of the last one
// (0: none).
`timescale 1ns/1ns
module na(input c, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $nochange(negedge c, d, -2, -3, n);
  endspecify
endmodule
module nb(input c, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $nochange(negedge c, d, 2, -3, n);
  endspecify
endmodule
module ne(input c, input d);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $nochange(negedge c, d, -2, 3, n);
  endspecify
endmodule
module ng(input C, input F, input E);
  reg nf, n;
  wire dC, dF;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $setuphold(posedge C, F, -5, 10, nf, , , dC, dF);
    $nochange(posedge C, E, 0, 0, n);
  endspecify
endmodule
module tb;
  reg [10:0] c, d;
  reg [2:0] gc, gf, ge;
  // Window (102, 117), decided at the trailing edge.
  na a0(c[0], d[0]);
  na a1(c[1], d[1]);
  na a2(c[2], d[2]);
  // Window (98, 117), decided at the trailing edge.
  nb b0(c[3], d[3]);
  nb b1(c[4], d[4]);
  nb b2(c[5], d[5]);
  nb b3(c[6], d[6]);
  // Window (102, 123), decided at each data event.
  ne e0(c[7], d[7]);
  ne e1(c[8], d[8]);
  ne e2(c[9], d[9]);
  ne e3(c[10], d[10]);
  // Window (200, 220) on the undelayed signals.
  ng g0(gc[0], gf[0], ge[0]);
  ng g1(gc[1], gf[1], ge[1]);
  ng g2(gc[2], gf[2], ge[2]);
  initial begin
    c = 11'h7ff;
    d = 11'h000;
    #100 c = 11'h000;
    #20 c = 11'h7ff;
  end
  initial begin #105 d[0] = 1'b1; #5 d[0] = 1'b0; #8 d[0] = 1'b1; end
  initial begin #103 d[1] = 1'b1; #14 d[1] = 1'b0; end
  initial begin #101 d[2] = 1'b1; end
  initial begin #97 d[3] = 1'b1; #2 d[3] = 1'b0; end
  initial begin #116 d[4] = 1'b1; #1 d[4] = 1'b0; end
  initial begin #99 d[5] = 1'b1; end
  initial begin #98 d[6] = 1'b1; end
  initial begin #101 d[7] = 1'b1; #1 d[7] = 1'b0; end
  initial begin #103 d[8] = 1'b1; end
  initial begin #121 d[9] = 1'b1; #1 d[9] = 1'b0; end
  initial begin #123 d[10] = 1'b1; end
  initial begin
    gc = 3'b000;
    gf = 3'b000;
    ge = 3'b000;
    #200 gc = 3'b111;
    #20 gc = 3'b000;
  end
  initial begin #203 ge[0] = 1'b1; end
  initial begin #223 ge[1] = 1'b1; end
  initial begin #219 ge[2] = 1'b1; end
  initial begin
    #300;
    $display("a: %0d@%0d %0d@%0d %0d@%0d", a0.count, a0.last, a1.count, a1.last,
             a2.count, a2.last);
    $display("b: %0d@%0d %0d@%0d %0d@%0d %0d@%0d", b0.count, b0.last, b1.count, b1.last,
             b2.count, b2.last, b3.count, b3.last);
    $display("e: %0d@%0d %0d@%0d %0d@%0d %0d@%0d", e0.count, e0.last, e1.count, e1.last,
             e2.count, e2.last, e3.count, e3.last);
    $display("g: %0d@%0d %0d@%0d %0d@%0d", g0.count, g0.last, g1.count, g1.last,
             g2.count, g2.last);
    $finish;
  end
endmodule
