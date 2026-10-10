// SIM-035: sampled-value functions on complex clocking events (IEEE
// 1800-2009 16.9.3, 9.4.2, 14.12-14.14): event lists, expression edges,
// value changes, `edge`, named and clocking-block events, `iff` clocks and
// gates, and default/global clocking with event lists.
module sub (
    input logic p,
    input logic q,
    input logic [3:0] s
);
  default clocking dc @(posedge p or posedge q);
  endclocking
  initial #15 $display("D %h %h %h %b", $past(s), $past(s, 2), $past(s, 3), $fell(s[0]));
endmodule

module sub_named (
    input logic q,
    input logic [3:0] s
);
  clocking dk @(posedge q);
  endclocking
  default clocking dk;
  initial #16 $display("K %h %h", $past(s), $past(s, 2));
endmodule

module tb;
  logic a = 1'b0, b = 1'b1, c = 1'b0, k = 1'b0, g = 1'b0;
  logic [3:0] v = 4'h0;
  event e;
  clocking cbk @(posedge k);
  endclocking
  clocking cbi @(posedge k iff g);
  endclocking
  global clocking gc @(posedge a or negedge b);
  endclocking

  sub u_sub (
      .p(a),
      .q(k),
      .s(v)
  );
  sub_named u_named (
      .q(k),
      .s(v)
  );

  initial begin
    #1 v = 4'h1;
    #1 a = 1'b1;
    #1 v = 4'h2;
    #1 b = 1'b0;
    #1 c = 1'b1;
    #1 v = 4'h3;
    #1 a = 1'b0;
    #1 ->e;
    #1 v = 4'h4;
    #1 k = 1'b1;
    #1 begin
      k = 1'b0;
      g = 1'b1;
      v = 4'h5;
    end
    #1 k = 1'b1;
    #1 begin
      k = 1'b0;
      v = 4'h6;
    end
    #1 begin
      $display("or %h %h %h %b", $past(v, 1, , @(posedge a or negedge b)),
               $past(v, 2, , @(posedge a or negedge b)),
               $past(v, 3, , @(posedge a or negedge b)), $changed(v, @(posedge a or negedge b)));
      $display("expr %h %h", $past(v, 1, , @(posedge (a | ~b))),
               $past(v, 2, , @(posedge (a | ~b))));
      $display("change %h %h", $past(v, 1, , @(c)), $past(v, 2, , @(c)));
      $display("edge %h %h", $past(v, 1, , @(edge a)), $past(v, 2, , @(edge a)));
      $display("event %h %b", $past(v, 1, , @(e)), $stable(v, @(e)));
      $display("cb %h %h %b", $past(v, 1, , @(cbk)), $past(v, 2, , @(cbk)), $rose(v[0], @(cbk)));
      $display("cbiff %h %h", $past(v, 1, , @(cbi)), $past(v, 2, , @(cbi)));
      $display("iff %h %h %h %h", $past(v, 1, , @(posedge k iff g)),
               $past(v, 2, , @(posedge k iff g)), $past(v, 1, g, @(posedge k)),
               $past(v, 2, g, @(posedge k)));
      $display("gated-list %h %h", $past(v, 1, g, @(posedge k or posedge a)),
               $past(v, 2, g, @(posedge k or posedge a)));
      $display("gclk %h %b", $past_gclk(v), $changed_gclk(v));
    end
    #3 $finish;
  end
endmodule
