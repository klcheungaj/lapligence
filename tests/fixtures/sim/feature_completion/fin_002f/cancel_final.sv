// Actions taken in final procedures (9.2.3).
`timescale 1ns/1ns
module sub(input int id);
  final $display("sub %0d final", id);
endmodule
module tb;
  string act;
  process p, q;
  event e;
  int done;
  sub s1(1);
  sub s2(2);
  function automatic int deep(int n);
    if (n > 0) return deep(n - 1) + 1;
    case (act)
      "finish": $finish;
      "fatal": $fatal(1, "in final");
      "stop": $stop;
      "error": $error("in final");
      "killself": begin process me; me = process::self(); if (me == null) $display("no self"); else me.kill(); end
      "killother": begin p.kill(); q.kill(); $display("killed %s", p.status().name()); end
      "disable": disable tb.first;

      "disable_proc": disable tb.waiting;
      "null": begin process none; none.kill(); end
      "resume": begin p.resume(); $display("resumed"); end

      default: ;
    endcase
    $display("deep went on");
    return 0;
  endfunction
  task waiting;
    p = process::self();
    @e done++;
  endtask
  initial begin
    void'($value$plusargs("act=%s", act));
    fork waiting; begin q = process::self(); #100 done++; end join_none
    #5 $finish;
  end
  final begin : first
    $display("first");
    begin : blk
      int r;
      r = deep(2);
      $display("r=%0d", r);
    end
    $display("after blk");
  end
  final begin $display("second done=%0d p=%s q=%s", done, p.status().name(), q.status().name()); end
  sub s3(3);
endmodule
