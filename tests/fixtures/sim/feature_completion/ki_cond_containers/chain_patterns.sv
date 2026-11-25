// Pattern predicates whose identifiers the alternatives use, over conditionals
// of one and of five levels (IEEE 1800-2009 11.4.11, 12.6).
module tb;
  typedef struct { int a; string t; } rec_t;
  typedef union tagged { int Num; string Text; void None; } u_t;
  typedef int q_t[$];
  rec_t r0 = '{0, "s"}, r1 = '{1, "s"}, r2 = '{2, "t"}, ry;
  q_t q0 = '{0, 9}, q1 = '{1, 9}, q2 = '{2, 8}, qy;
  u_t u, v;
  logic x;
  function automatic rec_t make(int a, string t);
    $write("  make(%0d, %s)", a, t);
    return '{a, t};
  endfunction
  task automatic show(input string what);
    $display("%s: %0d <%s> %p", what, ry.a, ry.t, qy);
  endtask
  task automatic each();
    ry = u matches tagged Num .n &&& n > 2 ? make(n, "num") : r1;
    qy = u matches tagged Num .n ? (n > 2 ? '{n, 9} : q1) : q2;
    show("one");
    ry = u matches tagged Num .n &&& n > 2 ? make(n, "num") :
         x ? r0 :
         v matches tagged Num .n &&& n < 9 ? make(n, "v") :
         u matches tagged Text .t ? make(7, t) :
         v matches tagged Text .t ? make(8, t) : r2;
    qy = u matches tagged Num .n &&& n > 2 ? '{n, 9} :
         x ? q0 :
         v matches tagged Num .n &&& n < 9 ? '{n, 9} :
         u matches tagged Text .t ? q1 :
         v matches tagged Text .t ? '{8} : q2;
    show("chain");
    ry = v matches tagged Num .n ? (x ? (u matches tagged Num .m ? (n > 4 ? make(n + m, "both") : r0) : make(n, "n")) : r1) : r2;
    qy = v matches tagged Num .n ? (x ? (u matches tagged Num .m ? (n > 4 ? '{n + m, 9} : q0) : '{n, 9}) : q1) : q2;
    show("first");
  endtask
  initial begin
    for (int i = 0; i < 36; i++) begin
      case (i % 4)
        0: u = tagged None;
        1: u = tagged Num (3);
        2: u = tagged Num (1);
        3: u = tagged Text ("a");
      endcase
      case ((i / 4) % 3)
        0: v = tagged None;
        1: v = tagged Num (5);
        2: v = tagged Text ("b");
      endcase
      case (i / 12)
        0: x = 0;
        1: x = 1;
        2: x = 'x;
      endcase
      $display("%0d", i);
      each();
    end
  end
endmodule
