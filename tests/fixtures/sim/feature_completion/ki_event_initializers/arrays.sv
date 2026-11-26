// Declaration initializers of arrays of events: static fixed arrays (nested,
// keyed and default patterns, a whole array), a queue, and fixed arrays that
// are instance and static class properties.
`timescale 1ns / 1ns
module tb;
  event e1, e2;
  event ea[1:0][2] = '{'{e1, null}, '{default: e2}};
  event eb[2] = '{1: e1, default: e2};
  event ec[2] = eb;
  event dq[$] = '{e1, e2};

  class c;
    event ce[2] = '{e1, e2};
    static event se[2] = '{e2, e1};
  endclass

  c o;

  initial begin
    o = new;
    $display("%b %b %b %b", ea[1][0] == e1, ea[1][1] == null, ea[0][0] == e2, ea[0][1] == e2);
    $display("%b %b %b %b", eb[0] == e2, eb[1] == e1, ec[0] == e2, ec[1] == e1);
    $display("%b %b %0d", dq[0] == e1, dq[1] == e2, dq.size());
    $display("%b %b %b %b", o.ce[0] == e1, o.ce[1] == e2, c::se[0] == e2, c::se[1] == e1);
    fork
      begin
        @(ea[1][0]);
        $display("ea[1][0] %0t", $time);
      end
      begin
        @(ec[0]);
        $display("ec[0] %0t", $time);
      end
    join_none
    #1 -> e1;
    #1 -> e2;
    #1 $finish;
  end
endmodule
